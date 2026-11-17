//! The play log: which songs played today, as minute intervals for the welcome run's song band.
//! Pure model. The app feeds it media snapshots and saves the text form.

use std::fmt::Write as _;

/// A calendar day as `(year, month, day)`.
pub type Day = (u16, u16, u16);

/// The shortest play, in seconds, that is kept.
pub const MIN_PLAY_S: u32 = 10;

/// Seconds in a day.
const DAY_S: u32 = 1440 * 60;

/// A pause shorter than this, in seconds, does not split a play of the same track.
const MERGE_GAP_S: u32 = 60;

/// One played interval of one track. Minutes count from local midnight of the log's day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Play {
    /// First minute of the play (floored).
    pub start_min: u32,
    /// Minute the play ended (ceiled, at most 1440).
    pub end_min: u32,
    /// Track title, tabs and line breaks replaced by spaces.
    pub title: String,
    /// Track artist, tabs and line breaks replaced by spaces.
    pub artist: String,
}

/// The interval being played right now.
#[derive(Debug, Clone)]
struct Open {
    start_s: u32,
    play: Play,
}

/// Seconds span of the last kept play, so a quick resume can extend it.
#[derive(Debug, Clone, Copy)]
struct LastSpan {
    start_s: u32,
    end_s: u32,
}

/// Today's plays.
#[derive(Debug, Clone, Default)]
pub struct PlayLog {
    day: Day,
    plays: Vec<Play>,
    last: Option<LastSpan>,
    open: Option<Open>,
}

fn clean(text: &str) -> String {
    text.replace(['\t', '\r', '\n'], " ")
}

fn ceil_min(seconds: u32) -> u32 {
    seconds.div_ceil(60).min(1440)
}

type Wanted = Option<(String, String)>;

impl PlayLog {
    /// Feeds one media snapshot. `second` is 0..59 within `minute_of_day`.
    pub fn observe(
        &mut self,
        day: Day,
        minute_of_day: u32,
        second: u32,
        track: Option<(&str, &str)>,
        playing: bool,
    ) {
        let now = (minute_of_day * 60 + second).min(DAY_S);
        let want: Wanted = track
            .filter(|_| playing)
            .map(|(title, artist)| (clean(title), clean(artist)));
        if day != self.day {
            self.roll_day(day, want.as_ref(), now);
        }
        if let (Some(open), Some((title, artist))) = (self.open.as_mut(), want.as_ref())
            && open.play.title == *title
            && open.play.artist == *artist
        {
            open.play.end_min = ceil_min(now).max(open.play.start_min + 1);
            return;
        }
        self.close(now);
        if let Some((title, artist)) = want {
            self.open_play(title, artist, now);
        }
    }

    fn roll_day(&mut self, day: Day, want: Option<&(String, String)>, now: u32) {
        self.day = day;
        self.plays.clear();
        self.last = None;
        let same = self
            .open
            .as_ref()
            .is_some_and(|o| want.is_some_and(|(t, a)| o.play.title == *t && o.play.artist == *a));
        if same {
            if let Some(open) = self.open.as_mut() {
                open.start_s = 0;
                open.play.start_min = 0;
                open.play.end_min = ceil_min(now).max(1);
            }
        } else {
            self.open = None;
        }
    }

    fn close(&mut self, now: u32) {
        let Some(mut open) = self.open.take() else {
            return;
        };
        if now.saturating_sub(open.start_s) < MIN_PLAY_S {
            return;
        }
        open.play.end_min = ceil_min(now);
        self.last = Some(LastSpan {
            start_s: open.start_s,
            end_s: now,
        });
        self.plays.push(open.play);
    }

    fn open_play(&mut self, title: String, artist: String, now: u32) {
        if let (Some(last), Some(prev)) = (self.last, self.plays.last())
            && prev.title == title
            && prev.artist == artist
            && now.saturating_sub(last.end_s) <= MERGE_GAP_S
            && let Some(play) = self.plays.pop()
        {
            self.last = None;
            self.open = Some(Open {
                start_s: last.start_s,
                play: Play {
                    end_min: ceil_min(now).max(play.start_min + 1),
                    ..play
                },
            });
            return;
        }
        let start_min = now / 60;
        self.open = Some(Open {
            start_s: now,
            play: Play {
                start_min,
                end_min: ceil_min(now).max(start_min + 1),
                title,
                artist,
            },
        });
    }

    /// The interval playing now, ending at the latest observed minute.
    #[must_use]
    pub fn current(&self) -> Option<&Play> {
        self.open.as_ref().map(|o| &o.play)
    }

    /// Plays (closed and open) that overlap the minutes `[from_min, to_min)`.
    #[must_use]
    pub fn in_window(&self, from_min: i32, to_min: i32) -> Vec<&Play> {
        self.plays
            .iter()
            .chain(self.current())
            .filter(|p| {
                i64::from(p.start_min) < i64::from(to_min)
                    && i64::from(p.end_min) > i64::from(from_min)
            })
            .collect()
    }

    /// The text form: one `YYYY-MM-DD start end title<TAB>artist` line per play.
    #[must_use]
    pub fn serialize(&self) -> String {
        let (y, m, d) = self.day;
        let mut out = String::new();
        for p in self.plays.iter().chain(self.current()) {
            let _ = writeln!(
                out,
                "{y:04}-{m:02}-{d:02} {} {} {}\t{}",
                p.start_min, p.end_min, p.title, p.artist
            );
        }
        out
    }

    /// Reads the text form. Malformed lines are skipped; only the most recent day is kept.
    /// The result has no open interval.
    #[must_use]
    pub fn parse(text: &str) -> PlayLog {
        let lines: Vec<(Day, Play)> = text.lines().filter_map(parse_line).collect();
        let day = lines.iter().map(|(d, _)| *d).max().unwrap_or_default();
        PlayLog {
            day,
            plays: lines
                .into_iter()
                .filter(|(d, _)| *d == day)
                .map(|(_, p)| p)
                .collect(),
            last: None,
            open: None,
        }
    }
}

fn parse_line(line: &str) -> Option<(Day, Play)> {
    let mut parts = line.splitn(4, ' ');
    let mut date = parts.next()?.split('-');
    let day = (
        date.next()?.parse().ok()?,
        date.next()?.parse().ok()?,
        date.next()?.parse().ok()?,
    );
    if date.next().is_some() {
        return None;
    }
    let start_min: u32 = parts.next()?.parse().ok()?;
    let end_min: u32 = parts.next()?.parse().ok()?;
    let (title, artist) = parts.next()?.split_once('\t')?;
    (end_min > start_min && end_min <= 1440).then(|| {
        (
            day,
            Play {
                start_min,
                end_min,
                title: title.to_owned(),
                artist: artist.to_owned(),
            },
        )
    })
}

#[cfg(test)]
mod tests;
