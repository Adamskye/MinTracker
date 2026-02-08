use std::{
    cell::RefCell,
    f32::consts::PI,
    sync::{
        mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError},
        Arc, LockResult, RwLock, RwLockReadGuard,
    },
    time::{Duration, Instant},
};

use rodio::{OutputStream, OutputStreamBuilder, Sink, Source};

use crate::{
    helpers,
    project::{
        InstrumentDataTable, InstrumentVariant, NoteEffects, Project, ProjectLocation,
        ProjectSettings, SlideEffect, TrackSettings, VOICES_PER_TRACK,
    },
};

pub const SAMPLE_RATE: u32 = 44100;

pub struct ROProject {
    project: Arc<RwLock<Project>>,
}

impl ROProject {
    pub fn new(project: Arc<RwLock<Project>>) -> Self {
        Self { project }
    }

    pub fn read(&self) -> LockResult<RwLockReadGuard<'_, Project>> {
        self.project.read()
    }
}

pub enum PlayerCmd {
    UpdateBuffer(PlayerScope),
    Pause,
    Resume,
    Stop,
    Dummy,
    RequestIsPaused(Sender<bool>),
    RequestLocation(Sender<Vec<Option<ProjectLocation>>>),
    RequestScope(Sender<Arc<PlayerScope>>),
    RequestBuffer(Sender<Option<Arc<PlayerScope>>>),
}

#[derive(Clone)]
pub struct PlayerScope {
    pub first_notes: Arc<[ProjectLocation]>,
    pub last_note: Option<ProjectLocation>,
}

#[derive(Default)]
pub struct Player {
    playing: RefCell<Option<Sender<PlayerCmd>>>,
}

impl Player {
    // todo: use PlayerScope
    pub fn play(&self, project: ROProject, scope: PlayerScope) {
        let (tx, rx) = mpsc::channel();
        {
            *self.playing.borrow_mut() = Some(tx);
        }

        std::thread::spawn(move || {
            Self::play_thread(project, Arc::new(scope), rx);
        });
    }

    #[allow(dead_code)]
    fn is_playing_a_chain(
        start: Arc<[ProjectLocation]>,
        last_note: Option<&ProjectLocation>,
    ) -> bool {
        let Some(last_note) = last_note else {
            return false;
        };

        let Some(first_note) = start.first() else {
            return false;
        };

        first_note.track_idx == last_note.track_idx
            && first_note.chain_offset == last_note.chain_offset
            && first_note.phrase_offset <= last_note.phrase_offset
            && first_note.note_offset <= last_note.note_offset
    }

    #[allow(dead_code)]
    fn is_playing_a_phrase(
        start: Arc<[ProjectLocation]>,
        last_note: Option<&ProjectLocation>,
    ) -> bool {
        let Some(last_note) = last_note else {
            return false;
        };

        let Some(start) = start.first() else {
            return false;
        };

        start.track_idx == last_note.track_idx
            && start.chain_offset == last_note.chain_offset
            && start.phrase_offset == last_note.phrase_offset
            && start.note_offset <= last_note.note_offset
    }

    pub fn is_playing(&self) -> bool {
        let playing = &self.playing.borrow();
        let Some(tx) = playing.as_ref() else {
            return false;
        };

        if tx.send(PlayerCmd::Dummy).is_err() {
            return false;
        }

        true
    }

    pub fn send_command(&self, command: PlayerCmd) {
        let playing = self.playing.borrow();
        let Some(playing) = playing.as_ref() else {
            return;
        };
        let _ = playing.send(command);
    }

    fn play_thread(project: ROProject, mut scope: Arc<PlayerScope>, rx: Receiver<PlayerCmd>) {
        let note_pool = NotePool::default();
        let mut paused = false;

        let mut i: i64 = -1;

        let mut last_note_time = Instant::now();
        let mut project_settings = project.read().unwrap().settings().clone();

        // buffer - move onto this after reaching end
        let mut buffer: Option<Arc<PlayerScope>> = None;

        // if a position within a track is `None`, then the track is finished
        let mut current_pos = scope
            .first_notes
            .iter()
            .cloned()
            .map(Some)
            .collect::<Vec<Option<ProjectLocation>>>();

        let mut to_end = false;

        'main: loop {
            // handle commands
            let ms_per_note =
                helpers::ticks_to_duration(1.0, project.read().unwrap().settings().tempo)
                    .as_millis();

            loop {
                let timeout = ms_per_note.saturating_sub(last_note_time.elapsed().as_millis());

                match rx.recv_timeout(Duration::from_millis(timeout as u64)) {
                    Ok(PlayerCmd::UpdateBuffer(scope)) => {
                        buffer = Some(scope.into());
                    }
                    Ok(PlayerCmd::Stop) => break 'main,
                    Ok(PlayerCmd::Pause) => paused = true,
                    Ok(PlayerCmd::Resume) => paused = false,
                    Ok(PlayerCmd::RequestIsPaused(tx)) => {
                        let _ = tx.send(paused);
                    }
                    Ok(PlayerCmd::RequestLocation(tx)) => {
                        let _ = tx.send(current_pos.clone());
                    }
                    Ok(PlayerCmd::RequestScope(tx)) => {
                        let _ = tx.send(scope.clone());
                    }
                    Ok(PlayerCmd::RequestBuffer(tx)) => {
                        let _ = tx.send(buffer.clone());
                    }
                    Ok(PlayerCmd::Dummy) => (),
                    Err(RecvTimeoutError::Timeout) => (),
                    Err(RecvTimeoutError::Disconnected) => break 'main,
                };
                if !paused {
                    break;
                }
            }

            // update project settings
            if *project.read().unwrap().settings() != project_settings {
                project_settings = project.read().unwrap().settings().clone();
                note_pool.send_message_to_all(OscillatorMsg::SetProjectSettings(
                    project.read().unwrap().settings().clone(),
                ));
            }

            if last_note_time.elapsed().as_millis() < ms_per_note {
                continue;
            }

            if to_end {
                break;
            }

            // start playing note
            i = i.saturating_add(1);

            let Ok(project) = project.read() else {
                break;
            };

            let mut all_tracks_finished = true;

            // go through each track and play the notes
            for track_location_opt in current_pos.iter_mut() {
                let Some(track_location) = track_location_opt else {
                    continue;
                };

                all_tracks_finished = false;

                for voice_idx in 0..VOICES_PER_TRACK {
                    note_pool.play_note(voice_idx, &project, track_location);
                }

                let new_location_opt = project.increment_project_location(*track_location);
                match new_location_opt {
                    Some(new_location) if Some(*track_location) != scope.last_note => {
                        *track_location = new_location
                    }
                    _ => {
                        *track_location_opt = None;
                    }
                };
            }

            // should end?
            // if all_tracks_finished, move onto buffer. If there's no buffer, loop if loop_player
            // setting is enabled.
            if all_tracks_finished {
                if let Some(new_scope) = buffer.take() {
                    scope = new_scope;
                    current_pos
                        .iter_mut()
                        .zip(scope.first_notes.iter())
                        .for_each(|(current_pos, start_pos)| *current_pos = Some(*start_pos));
                } else if project_settings.loop_player {
                    current_pos
                        .iter_mut()
                        .zip(scope.first_notes.iter())
                        .for_each(|(current_pos, start_pos)| *current_pos = Some(*start_pos));
                } else {
                    to_end = true;
                }
            } else {
                last_note_time = Instant::now();
            }
        }
    }
}

pub struct NotePool {
    tracklist: RefCell<Vec<[Option<Sender<OscillatorMsg>>; VOICES_PER_TRACK]>>, // tracklist[track][voice]
    stream: OutputStream,
}

impl Default for NotePool {
    fn default() -> Self {
        // TODO: do get rid of unwrap here
        let stream = OutputStreamBuilder::open_default_stream().unwrap();
        Self {
            tracklist: Default::default(),
            stream,
        }
    }
}

impl NotePool {
    pub fn play_note(&self, voice: usize, project: &Project, location: &ProjectLocation) {
        if voice >= VOICES_PER_TRACK {
            return;
        }

        // fetch copy of note
        let Some(mut note) = project
            .get_notes_at_location(*location)
            .and_then(|notes| notes.get(voice).map(|note| (*note).clone()))
        else {
            return;
        };

        // fetch reference to track
        let Some(track) = project.tracks().get(location.track_idx) else {
            return;
        };

        let instrument_opt = track
            .settings
            .instrument
            .and_then(|inst_id| project.instruments().get(&inst_id));

        let (Some(semitone), Some(instrument)) = (note.semitone(), instrument_opt) else {
            // if note does not have an assigned semitone but does have effects
            if note.has_effects() {
                self.send_message(
                    location.track_idx,
                    voice,
                    OscillatorMsg::AddEffects(note.effects),
                );
            }
            return;
        };

        // attach start and end semitones if the note has a slide effect
        let semitone_opt = note.semitone();
        if let Some(slide_effect) = &mut note.effects.slide {
            let (tx, rx) = mpsc::channel();

            // end semitone
            slide_effect.end_semitone = semitone_opt;

            // start semitone
            self.send_message(location.track_idx, voice, OscillatorMsg::GetFrequency(tx));
            slide_effect.start_semitone = rx.recv().ok().map(helpers::semitone_from_frequency);
        }

        // access tracklist and resize if needed
        let mut tracklist = self.tracklist.borrow_mut();
        if tracklist.len() <= location.track_idx {
            tracklist.resize_with(location.track_idx + 1, Default::default);
        }

        let Some(data_table) = instrument
            .data_table_map
            .get(semitone as usize)
            .cloned()
            .flatten()
            .and_then(|index| instrument.data_tables.get(index))
            .cloned()
        else {
            return;
        };

        let (wo, tx) = WavetableOscillator::with_frequency(
            data_table,
            helpers::frequency_from_semitone(
                semitone as f32
                    + project
                        .get_note_transpose_semitones(*location)
                        .unwrap_or(0.0),
            ),
        );
        let wo = wo.stoppable().amplify(0.2);

        let sink = Sink::connect_new(self.stream.mixer());
        sink.append(wo);
        tracklist[location.track_idx][voice] = Some(tx);

        if let Some(tx) = &tracklist[location.track_idx][voice] {
            let _ = tx.send(OscillatorMsg::AddEffects(note.effects));
            let _ = tx.send(OscillatorMsg::SetProjectSettings(
                project.settings().clone(),
            ));
            let _ = tx.send(OscillatorMsg::SetTrackSettings(track.settings.clone()));
        }

        sink.play();
        sink.detach();
    }

    pub fn send_message(&self, track: usize, voice: usize, message: OscillatorMsg) {
        let mut tracklist = self.tracklist.borrow_mut();
        if tracklist.len() <= track {
            tracklist.resize_with(track + 1, Default::default);
        }

        if let Some(tx) = &tracklist[track][voice] {
            let _ = tx.send(message);
        }
    }

    pub fn send_message_to_all(&self, message: OscillatorMsg) {
        self.tracklist
            .borrow_mut()
            .iter_mut()
            .flatten()
            .filter_map(|voice| voice.as_mut())
            .for_each(|tx| {
                let _ = tx.send(message.clone());
            });
    }

    #[allow(dead_code)]
    pub fn stop_note(&self, track: usize, voice: usize) {
        if let Some(voice) = self
            .tracklist
            .borrow_mut()
            .get_mut(track)
            .and_then(|t| t.get_mut(voice))
        {
            *voice = None;
        }
    }
}

#[derive(Clone)]
pub enum OscillatorMsg {
    AddEffects(NoteEffects),
    GetFrequency(Sender<f32>),
    SetProjectSettings(ProjectSettings),
    SetTrackSettings(TrackSettings),
}

pub struct WavetableOscillator {
    /// instrument data
    data_table: Arc<InstrumentDataTable>,
    /// current location in wavetable
    index: f32,
    /// playing frequency (before pitch bend/vibrato/etc.)
    frequency: f32,
    /// counts up once every time a sample is fetched
    counter: u64,
    /// updated when calculating volume modifier due to volume envelope, and used when getting the
    /// envelope volume modifier for the right ear (which would be the same as the left ear)
    prev_env_vol: f32,

    project_settings: ProjectSettings,
    track_settings: TrackSettings,

    /// receives messages, such as to add effects, set the project settings or pan
    rx: Receiver<OscillatorMsg>,
    /// time between note beginning and note ending, and is used to calculate release volume
    time_when_stopped: Option<f32>,

    /// alternates between true and false, where true means that the left ear is playing
    stereo_first_ear: bool,

    /// contains all effects
    effects: NoteEffects,
    /// sample counter that starts when vibrato effect begins
    vibrato_counter: u64,
    /// sample counter that starts when kill effect begins
    kill_counter: u64,
    /// sample counter that starts when soft kill effect begins
    soft_kill_counter: u64,
    /// sample counter that starts when pitch bend begins
    pitch_bend_counter: u64,
    /// sample counter that starts when slide effect begins
    slide_counter: u64,
}

impl WavetableOscillator {
    pub fn new(data_table: Arc<InstrumentDataTable>) -> (Self, Sender<OscillatorMsg>) {
        let (tx, rx) = mpsc::channel();
        (
            Self {
                data_table,
                index: 0.0,
                frequency: 0.0,
                rx,
                time_when_stopped: None,
                counter: 0,
                prev_env_vol: 0.0,

                project_settings: Default::default(),
                track_settings: Default::default(),

                // effects
                effects: NoteEffects::default(),
                vibrato_counter: 0,
                kill_counter: 0,
                soft_kill_counter: 0,
                pitch_bend_counter: 0,
                slide_counter: 0,

                stereo_first_ear: true,
            },
            tx,
        )
    }

    pub fn with_frequency(
        data_table: Arc<InstrumentDataTable>,
        frequency: f32,
    ) -> (Self, Sender<OscillatorMsg>) {
        let (mut wo, tx) = Self::new(data_table);
        wo.set_frequency(frequency);
        (wo, tx)
    }

    pub fn set_frequency(&mut self, frequency: f32) {
        self.frequency = frequency;
    }

    fn frequency_to_increment(&self, frequency: f32) -> f32 {
        frequency * self.data_table.data.len() as f32 / SAMPLE_RATE as f32
    }

    fn handle_messages(&mut self) {
        loop {
            match self.rx.try_recv() {
                Ok(OscillatorMsg::AddEffects(fx)) => {
                    self.effects.add_from_other(&fx);
                }
                Ok(OscillatorMsg::SetProjectSettings(new_settings)) => {
                    self.project_settings = new_settings;
                }
                Ok(OscillatorMsg::SetTrackSettings(settings)) => {
                    self.track_settings = settings;
                }
                Ok(OscillatorMsg::GetFrequency(tx)) => {
                    let _ = tx.send(self.frequency);
                }
                Err(TryRecvError::Disconnected) => {
                    if self.time_when_stopped.is_none() {
                        let time_ms = (self.counter as f32 / SAMPLE_RATE as f32) * 1000.0;
                        self.time_when_stopped = Some(time_ms);
                    }
                    break;
                }
                Err(TryRecvError::Empty) => break,
            };
        }
    }

    fn sample_counter_to_ms(count: u64) -> f32 {
        (count as f32 / SAMPLE_RATE as f32) * 1000.0
    }

    fn get_increment(&mut self) -> f32 {
        // should only increment when fetching a new sample
        if !self.should_fetch_new_sample() {
            return 0.0;
        }

        let inc = match self.data_table.variant {
            InstrumentVariant::Normal => {
                // vibrato
                let vibrato_mult = match &self.effects.vibrato {
                    Some(v) => {
                        let time_ms = Self::sample_counter_to_ms(self.vibrato_counter);
                        let semitone_diff =
                            f32::sin((2.0 * PI * time_ms) / v.speed as f32) * v.amplitude;
                        let multiplier = 2.0_f32.powf(semitone_diff / 12.0);
                        self.vibrato_counter = self.vibrato_counter.saturating_add(1);
                        multiplier
                    }
                    None => 1.0,
                };

                // pitch bend
                let pitch_bend_mult = match &self.effects.pitch_bend {
                    Some(p) if p.semitones_per_tick != 0.0 => {
                        let time_ms = Self::sample_counter_to_ms(self.pitch_bend_counter);
                        let ms_per_semitone = helpers::ticks_to_duration(
                            1.0 / p.semitones_per_tick.abs(),
                            self.project_settings.tempo,
                        )
                        .as_millis();

                        let multiplier = if ms_per_semitone == 0 {
                            1.0
                        } else {
                            let semitone_diff = time_ms / ms_per_semitone as f32;
                            2.0_f32.powf(
                                semitone_diff * (p.semitones_per_tick / p.semitones_per_tick.abs())
                                    / 12.0,
                            )
                        };
                        self.pitch_bend_counter = self.pitch_bend_counter.saturating_add(1);
                        multiplier
                    }
                    _ => 1.0,
                };

                // slide
                let slide_mult = match self.effects.slide {
                    Some(SlideEffect {
                        start_semitone: Some(start_semitone),
                        end_semitone: Some(end_semitone),
                        time_ticks,
                    }) => 'block: {
                        // get number of samples between start and end of slide
                        let total_samples =
                            helpers::ticks_to_duration(time_ticks, self.project_settings.tempo)
                                .as_secs_f64()
                                * SAMPLE_RATE as f64;

                        if total_samples <= 0. || self.slide_counter >= (total_samples as u64) {
                            break 'block 1.0;
                        }

                        let start_semitone = start_semitone as f64;
                        let end_semitone = end_semitone as f64;

                        let actual_semitone = start_semitone
                            + (((end_semitone - start_semitone) / total_samples)
                                * self.slide_counter as f64);

                        let actual_frequency =
                            helpers::frequency_from_semitone(actual_semitone as f32);

                        let multiplier = actual_frequency / self.frequency;

                        self.slide_counter = self.slide_counter.saturating_add(1);
                        multiplier
                    }
                    _ => 1.0,
                };

                // final increment
                self.frequency_to_increment(
                    self.frequency * pitch_bend_mult * vibrato_mult * slide_mult,
                )
            }
            InstrumentVariant::OneShot => 1.0,
            InstrumentVariant::OneShotPitched(freq) => self.frequency / freq,
        };

        self.counter = self.counter.saturating_add(1);
        inc
    }

    fn get_pan_modifier(&self) -> f32 {
        // pan effect
        let pan = match &self.effects.pan {
            Some(pan) => pan.value,
            None => self.track_settings.pan,
        };

        if self.stereo_first_ear {
            1.0 - pan
        } else {
            1.0 + pan
        }
        .min(1.0)
    }

    fn get_envelope_modifier(&self) -> Option<f32> {
        let envelope = if let Some(env_effect) = &self.effects.envelope {
            &env_effect.envelope
        } else {
            &self.data_table.new_envelope
        };

        let time_ms = Self::sample_counter_to_ms(self.counter);

        Some(if self.stereo_first_ear {
            if let Some(time_when_stopped) = self.time_when_stopped {
                envelope.volume_at_time_stopped(time_ms, time_when_stopped)?
            } else {
                envelope.volume_at_time(time_ms)
            }
        } else {
            self.prev_env_vol
        })
    }

    fn should_fetch_new_sample(&self) -> bool {
        self.data_table.variant != InstrumentVariant::Normal || self.stereo_first_ear
    }

    fn should_activate_kill_effect(&mut self) -> bool {
        if let Some(kill_effect) = &mut self.effects.kill {
            let duration_in_samples =
                helpers::ticks_to_duration(kill_effect.delay_ticks, self.project_settings.tempo)
                    .as_secs_f32()
                    * SAMPLE_RATE as f32;

            if self.kill_counter > duration_in_samples as u64 {
                return true;
            }

            if self.should_fetch_new_sample() {
                self.kill_counter = self.kill_counter.saturating_add(1);
            }
        }
        false
    }

    fn handle_soft_kill_effect(&mut self) {
        if self.time_when_stopped.is_some() {
            return;
        }

        if let Some(soft_kill_effect) = &mut self.effects.soft_kill {
            let duration_in_samples = helpers::ticks_to_duration(
                soft_kill_effect.delay_ticks,
                self.project_settings.tempo,
            )
            .as_secs_f32()
                * SAMPLE_RATE as f32;

            if self.soft_kill_counter > duration_in_samples as u64 {
                self.time_when_stopped = Some(Self::sample_counter_to_ms(self.counter));
                return;
            }

            if self.should_fetch_new_sample() {
                self.soft_kill_counter = self.soft_kill_counter.saturating_add(1);
            }
        }
    }
}

impl Iterator for WavetableOscillator {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        self.handle_messages();

        // if sample is finished
        if self.index >= self.data_table.data.len() as f32 {
            return None;
        }

        // kill effect
        if self.should_activate_kill_effect() {
            return None;
        }

        self.handle_soft_kill_effect();

        // getting volume modifier
        let env_vol = self.get_envelope_modifier()?;
        let pan_vol = self.get_pan_modifier();
        let muted = if self.track_settings.muted { 0.0 } else { 1.0 };
        let volume_modifier = muted * env_vol * pan_vol * self.track_settings.volume;

        // fetching sample (i.e. handling pitch)
        let increment = self.get_increment();
        let sample = helpers::linear_interpolate(&self.data_table.data, self.index);
        self.index += increment;

        // only loop data table if using a normal instrument
        if self.data_table.variant == InstrumentVariant::Normal {
            self.index %= self.data_table.data.len() as f32;
        }
        self.prev_env_vol = env_vol;

        // alternate this variable when switching between left and right ear
        self.stereo_first_ear = !self.stereo_first_ear;

        Some(sample.value() * volume_modifier)
    }
}

impl Source for WavetableOscillator {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

impl Drop for WavetableOscillator {
    fn drop(&mut self) {
        println!("N");
    }
}
