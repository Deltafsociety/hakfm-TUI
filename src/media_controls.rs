use crossterm::event::{KeyCode, MediaKeyCode};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use std::sync::mpsc::{self, Receiver};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaAction {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Stop,
}

impl MediaAction {
    pub fn from_key(code: KeyCode) -> Option<Self> {
        match code {
            KeyCode::Media(MediaKeyCode::Play) => Some(Self::Play),
            KeyCode::Media(MediaKeyCode::Pause) => Some(Self::Pause),
            KeyCode::Media(MediaKeyCode::PlayPause) => Some(Self::Toggle),
            KeyCode::Media(MediaKeyCode::TrackNext) => Some(Self::Next),
            KeyCode::Media(MediaKeyCode::TrackPrevious) => Some(Self::Previous),
            KeyCode::Media(MediaKeyCode::Stop) => Some(Self::Stop),
            _ => None,
        }
    }

    fn from_event(event: MediaControlEvent) -> Option<Self> {
        match event {
            MediaControlEvent::Play => Some(Self::Play),
            MediaControlEvent::Pause => Some(Self::Pause),
            MediaControlEvent::Toggle => Some(Self::Toggle),
            MediaControlEvent::Next => Some(Self::Next),
            MediaControlEvent::Previous => Some(Self::Previous),
            MediaControlEvent::Stop => Some(Self::Stop),
            _ => None,
        }
    }
}

// Keep callbacks off the TUI state: the main loop consumes these commands.
pub struct SystemMediaControls {
    controls: MediaControls,
    events: Receiver<MediaAction>,
    published: Option<(String, String, MediaPlayback)>,
}

impl SystemMediaControls {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(target_os = "macos")]
        macos::initialize();

        let mut controls = MediaControls::new(PlatformConfig {
            dbus_name: "hakfm",
            display_name: "HAK.FM",
            hwnd: None,
        })?;
        let (tx, events) = mpsc::channel();
        controls.attach(move |event| {
            if let Some(action) = MediaAction::from_event(event) {
                let _ = tx.send(action);
            }
        })?;

        Ok(Self {
            controls,
            events,
            published: None,
        })
    }

    pub fn poll(&self) -> Option<MediaAction> {
        self.events.try_recv().ok()
    }

    pub fn update(
        &mut self,
        station: Option<&super::Station>,
        playback: MediaPlayback,
    ) -> Result<(), souvlaki::Error> {
        let title = station.map(|s| s.name.as_str()).unwrap_or("HAK.FM");
        let genre = station
            .map(|s| s.genre.as_str())
            .unwrap_or("Internet radio");
        let snapshot = (title.to_owned(), genre.to_owned(), playback.clone());
        if self.published.as_ref() == Some(&snapshot) {
            return Ok(());
        }
        self.controls.set_metadata(MediaMetadata {
            title: Some(title),
            artist: Some(genre),
            album: Some("HAK.FM · Live radio"),
            ..Default::default()
        })?;
        self.controls.set_playback(playback)?;
        self.published = Some(snapshot);
        Ok(())
    }
}

impl Drop for SystemMediaControls {
    fn drop(&mut self) {
        let _ = self.controls.set_playback(MediaPlayback::Stopped);
        let _ = self.controls.set_metadata(MediaMetadata::default());
        let _ = self.controls.detach();
    }
}

pub fn pump_events() {
    #[cfg(target_os = "macos")]
    macos::pump_events();
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::{rc::autoreleasepool, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSEventMask};
    use objc2_foundation::{NSDate, NSDefaultRunLoopMode};

    pub fn initialize() {
        let mtm = MainThreadMarker::new().expect("media controls must start on the main thread");
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Prohibited);
        app.finishLaunching();
    }

    // A terminal app still needs to service AppKit's main queue for remote commands.
    // Pump without opening a window, showing a Dock icon, or taking keyboard focus.
    pub fn pump_events() {
        autoreleasepool(|_| {
            let mtm = MainThreadMarker::new().expect("AppKit must run on the main thread");
            let app = NSApplication::sharedApplication(mtm);
            let until = NSDate::distantPast();
            // SAFETY: AppKit's immutable run-loop-mode constant is valid for the process lifetime.
            let mode = unsafe { NSDefaultRunLoopMode };
            while let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(
                NSEventMask::Any,
                Some(&until),
                mode,
                true,
            ) {
                app.sendEvent(&event);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_and_system_media_keys_have_the_same_actions() {
        for (key, event, expected) in [
            (
                MediaKeyCode::Play,
                MediaControlEvent::Play,
                MediaAction::Play,
            ),
            (
                MediaKeyCode::Pause,
                MediaControlEvent::Pause,
                MediaAction::Pause,
            ),
            (
                MediaKeyCode::PlayPause,
                MediaControlEvent::Toggle,
                MediaAction::Toggle,
            ),
            (
                MediaKeyCode::TrackNext,
                MediaControlEvent::Next,
                MediaAction::Next,
            ),
            (
                MediaKeyCode::TrackPrevious,
                MediaControlEvent::Previous,
                MediaAction::Previous,
            ),
            (
                MediaKeyCode::Stop,
                MediaControlEvent::Stop,
                MediaAction::Stop,
            ),
        ] {
            assert_eq!(MediaAction::from_key(KeyCode::Media(key)), Some(expected));
            assert_eq!(MediaAction::from_event(event), Some(expected));
        }
    }

    #[test]
    fn unrelated_keys_and_unsupported_commands_are_ignored() {
        assert_eq!(MediaAction::from_key(KeyCode::Char('p')), None);
        assert_eq!(
            MediaAction::from_key(KeyCode::Media(MediaKeyCode::MuteVolume)),
            None
        );
        assert_eq!(MediaAction::from_event(MediaControlEvent::Raise), None);
    }
}
