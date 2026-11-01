use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub struct SleepTimer {
    deadline: Instant,
}

impl SleepTimer {
    pub fn new(duration: Duration, now: Instant) -> Result<Self, &'static str> {
        let deadline = now
            .checked_add(duration)
            .ok_or("That duration is too large.")?;
        Ok(Self { deadline })
    }

    pub fn expired(&self, now: Instant) -> bool {
        now >= self.deadline
    }

    pub fn label(&self, now: Instant) -> String {
        let remaining = self.deadline.saturating_duration_since(now);
        // Round up so a running timer never displays 00:00 before it expires.
        let seconds = remaining.as_secs() + u64::from(remaining.subsec_nanos() > 0);
        if seconds >= 3600 {
            format!(
                "{:02}:{:02}:{:02}",
                seconds / 3600,
                (seconds / 60) % 60,
                seconds % 60
            )
        } else {
            format!("{:02}:{:02}", seconds / 60, seconds % 60)
        }
    }
}

/// Bare numbers mean minutes. Unit-based input accepts combinations such as 1h30m.
/// Zero or "off" cancels the timer; an empty entry is an error.
pub fn parse_duration(input: &str) -> Result<Option<Duration>, &'static str> {
    const INVALID: &str = "Use minutes (30), or units: 90s, 30m, 1h30m. Use 0/off to cancel.";
    const TOO_LARGE: &str = "That duration is too large.";
    let input = input.trim().to_ascii_lowercase();
    if input == "off" || input == "0" {
        return Ok(None);
    }
    if input.is_empty() {
        return Err(INVALID);
    }
    if input.bytes().all(|c| c.is_ascii_digit()) {
        let minutes = input.parse::<u64>().map_err(|_| TOO_LARGE)?;
        let seconds = minutes.checked_mul(60).ok_or(TOO_LARGE)?;
        return Ok((seconds > 0).then(|| Duration::from_secs(seconds)));
    }

    let mut rest = input.as_str();
    let mut seconds = 0u64;
    while !rest.is_empty() {
        rest = rest.trim_start();
        let digits = rest.bytes().take_while(|c| c.is_ascii_digit()).count();
        if digits == 0 {
            return Err(INVALID);
        }
        let amount = rest[..digits].parse::<u64>().map_err(|_| TOO_LARGE)?;
        rest = rest[digits..].trim_start();
        let unit = rest.chars().next().ok_or(INVALID)?;
        let multiplier = match unit {
            's' => 1,
            'm' => 60,
            'h' => 3600,
            _ => return Err(INVALID),
        };
        seconds = seconds
            .checked_add(amount.checked_mul(multiplier).ok_or(TOO_LARGE)?)
            .ok_or(TOO_LARGE)?;
        rest = rest[unit.len_utf8()..].trim_start();
    }
    Ok((seconds > 0).then(|| Duration::from_secs(seconds)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_minutes_and_combined_units() {
        for (input, seconds) in [
            ("30", 1800),
            ("90s", 90),
            ("30m", 1800),
            ("1h30m", 5400),
            (" 1H 2M 3S ", 3723),
        ] {
            assert_eq!(
                parse_duration(input),
                Ok(Some(Duration::from_secs(seconds)))
            );
        }
        for input in ["0", "off", " OFF ", "0m", "00"] {
            assert_eq!(parse_duration(input), Ok(None));
        }
    }

    #[test]
    fn rejects_invalid_or_overflowing_durations() {
        for input in [
            "",
            " ",
            "-1",
            "1.5",
            "abc",
            "1h30",
            "1d",
            "1:30",
            "1m-1s",
            "1 0",
            "18446744073709551616",
            "18446744073709551615h",
            "18446744073709551615s1s",
        ] {
            assert!(parse_duration(input).is_err(), "accepted {input:?}");
        }
        assert!(SleepTimer::new(Duration::MAX, Instant::now()).is_err());
    }

    #[test]
    fn countdown_rounds_up_and_expires_at_the_deadline() {
        let now = Instant::now();
        let timer = SleepTimer::new(Duration::from_secs(90), now).unwrap();
        assert_eq!(timer.label(now), "01:30");
        assert_eq!(timer.label(now + Duration::from_millis(89_001)), "00:01");
        assert!(!timer.expired(now + Duration::from_millis(89_999)));
        assert!(timer.expired(now + Duration::from_secs(90)));
        assert_eq!(timer.label(now + Duration::from_secs(95)), "00:00");
        assert_eq!(
            SleepTimer::new(Duration::from_secs(5400), now)
                .unwrap()
                .label(now),
            "01:30:00"
        );
    }
}
