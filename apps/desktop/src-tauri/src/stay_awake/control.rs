//! AWK-001 to AWK-008.

use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use serde::Serialize;

use super::{
    hold::{self, Hold, LidHold},
    level::{Held, Level, Pause, Power, Wanted, held},
    lid_set_up::{self, Lid, SetUpError},
    platform::{Os, Platform},
    power, saved,
};

/// Stops a web-view script from showing the operator one password dialog after another.
const REFUSAL_REST: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    SetUp,
    Remove,
}

/// Runs with no lock on the control: the password dialog can stay open for minutes.
#[derive(Debug)]
pub struct Ask {
    platform: Platform,
    step: Step,
    set_sleep_back: bool,
}

impl Ask {
    pub fn run(&self) -> Result<(), SetUpError> {
        match self.step {
            Step::SetUp => lid_set_up::set_up(&self.platform),
            Step::Remove => lid_set_up::remove(&self.platform, self.set_sleep_back),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub level: Level,
    pub battery_floor: Option<u8>,
    pub held: Level,
    /// Why `held` is lower than `level`.
    pub pause: Option<Pause>,
    pub lid: Lid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ControlError {
    #[error("the choice could not be saved")]
    Unsaved,
    #[error(transparent)]
    SetUp(#[from] SetUpError),
}

#[derive(Debug)]
pub struct Control {
    platform: Platform,
    folder: PathBuf,
    wanted: Wanted,
    was_low: bool,
    idle: Option<Hold>,
    lid: Option<LidHold>,
    asking: bool,
    refused_at: Option<Instant>,
}

impl Control {
    /// Sets back a sleep setting that an earlier run left changed (AWK-007).
    #[must_use]
    pub fn open(platform: Platform, folder: PathBuf) -> Self {
        let _ = hold::restore(&platform, &saved::marker(&folder));
        Self {
            wanted: saved::load(&folder),
            platform,
            folder,
            was_low: false,
            idle: None,
            lid: None,
            asking: false,
            refused_at: None,
        }
    }

    #[must_use]
    pub fn active(&self) -> bool {
        self.wanted.level != Level::Off
    }

    fn power(&self) -> Power {
        if self.active() && self.wanted.battery_floor.is_some() {
            power::read(&self.platform)
        } else {
            Power::Mains
        }
    }

    fn hold_idle(&mut self) -> bool {
        if self.idle.as_mut().is_some_and(Hold::alive) {
            return true;
        }
        self.idle = hold::idle(&self.platform).ok();
        self.idle.is_some()
    }

    fn lid_holds(&mut self) -> bool {
        self.lid.as_mut().is_some_and(LidHold::alive)
    }

    fn hold_lid(&mut self) -> bool {
        if self.lid_holds() {
            return true;
        }
        self.lid = hold::lid(&self.platform, &saved::marker(&self.folder)).ok();
        self.lid.is_some()
    }

    fn drop_lid(&mut self) {
        if let Some(lid) = self.lid.take() {
            let _ = hold::release_lid(&self.platform, &saved::marker(&self.folder), lid);
        }
    }

    pub fn apply(&mut self, power: Power) -> Status {
        let lid = lid_set_up::lid(&self.platform);
        // No lid hold is taken while an administrator step runs: the rule can go at any moment.
        let lid_ready = lid == Lid::Ready && !self.asking;
        let mut target = held(self.wanted, power, lid_ready, self.was_low);
        if target.level == Level::LidClosed && !self.hold_lid() {
            target = held(self.wanted, power, false, self.was_low);
        }
        if target.level != Level::LidClosed {
            self.drop_lid();
        }
        if target.level == Level::Off {
            self.idle = None;
        } else if !self.hold_idle() {
            self.drop_lid();
            target = Held {
                level: Level::Off,
                pause: Some(Pause::Unavailable),
            };
        }
        self.was_low = target.pause == Some(Pause::LowBattery);
        Status {
            level: self.wanted.level,
            battery_floor: self.wanted.battery_floor,
            held: target.level,
            pause: target.pause,
            lid,
        }
    }

    pub fn refresh(&mut self) -> Status {
        self.apply(self.power())
    }

    fn change(&mut self, wanted: Wanted) -> Result<Status, ControlError> {
        saved::save(&self.folder, wanted).map_err(|_| ControlError::Unsaved)?;
        self.wanted = wanted;
        // The operator's own change ends a low-battery pause; the charge decides again.
        self.was_low = false;
        Ok(self.refresh())
    }

    pub fn choose(&mut self, level: Level) -> Result<Status, ControlError> {
        self.change(Wanted {
            level,
            ..self.wanted
        })
    }

    pub fn set_floor(&mut self, floor: Option<u8>) -> Result<Status, ControlError> {
        self.change(self.wanted.with_floor(floor))
    }

    /// A `Remove` sets the sleep setting back first, while the rule still lets it.
    pub fn begin(&mut self, step: Step, now: Instant) -> Result<Option<Ask>, ControlError> {
        if self.platform.os != Os::Mac {
            return Err(SetUpError::Unsupported.into());
        }
        let resting = self
            .refused_at
            .is_some_and(|at| now.duration_since(at) < REFUSAL_REST);
        if self.asking || resting {
            return Err(SetUpError::Refused.into());
        }
        let set_up = lid_set_up::lid(&self.platform) == Lid::Ready;
        if set_up == (step == Step::SetUp) {
            return Ok(None);
        }
        self.asking = true;
        if step == Step::Remove {
            self.drop_lid();
        }
        Ok(Some(Ask {
            platform: self.platform.clone(),
            step,
            set_sleep_back: saved::marker(&self.folder).exists(),
        }))
    }

    pub fn finish(
        &mut self,
        ask: &Ask,
        asked: Result<(), SetUpError>,
        now: Instant,
    ) -> Result<Status, ControlError> {
        self.asking = false;
        self.refused_at = asked.is_err().then_some(now);
        if asked.is_ok() && ask.set_sleep_back {
            // Root set the sleep setting back with the rule's removal.
            let _ = fs::remove_file(saved::marker(&self.folder));
        }
        let status = self.refresh();
        asked.map(|()| status).map_err(ControlError::from)
    }

    /// Ends every hold (AWK-007). The choice stays saved.
    pub fn release(&mut self) {
        self.idle = None;
        self.drop_lid();
    }
}
