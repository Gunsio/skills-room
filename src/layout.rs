use ratatui::layout::{Constraint, Layout, Rect};

pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 24;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LayoutTier {
    Compact,
    Standard,
    Wide,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct AppLayout {
    pub tier: LayoutTier,
    pub search: Rect,
    pub table: Rect,
    pub details: Rect,
    pub stats: Rect,
    pub output: Rect,
    pub help: Rect,
}

impl AppLayout {
    pub fn calculate(area: Rect) -> Option<Self> {
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            return None;
        }

        let tier = LayoutTier::for_size(area.width, area.height);
        let [search, body, output, help] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(tier.output_height()),
            Constraint::Length(2),
        ])
        .areas(area);

        let [table, side] = Layout::horizontal([
            Constraint::Percentage(tier.table_percent()),
            Constraint::Fill(1),
        ])
        .areas(body);

        let [details, stats] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(tier.stats_height())])
                .areas(side);

        Some(Self {
            tier,
            search,
            table,
            details,
            stats,
            output,
            help,
        })
    }
}

impl LayoutTier {
    fn for_size(width: u16, height: u16) -> Self {
        match (width, height) {
            (160.., 50..) => Self::Wide,
            (120.., 40..) => Self::Standard,
            _ => Self::Compact,
        }
    }

    fn table_percent(self) -> u16 {
        match self {
            Self::Compact => 62,
            Self::Standard => 60,
            Self::Wide => 64,
        }
    }

    fn stats_height(self) -> u16 {
        match self {
            Self::Compact => 5,
            Self::Standard => 7,
            Self::Wide => 9,
        }
    }

    fn output_height(self) -> u16 {
        match self {
            Self::Compact => 5,
            Self::Standard => 7,
            Self::Wide => 9,
        }
    }
}

pub fn too_small_message(area: Rect) -> String {
    format!(
        "Skillroom needs at least {MIN_WIDTH}x{MIN_HEIGHT}; current terminal is {}x{}.",
        area.width, area.height
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_too_small_terminal() {
        assert_eq!(AppLayout::calculate(Rect::new(0, 0, 79, 24)), None);
        assert_eq!(AppLayout::calculate(Rect::new(0, 0, 80, 23)), None);
    }

    #[test]
    fn supports_compact_standard_and_wide_breakpoints() {
        let cases = [
            ((80, 24), LayoutTier::Compact),
            ((120, 40), LayoutTier::Standard),
            ((160, 50), LayoutTier::Wide),
        ];

        for ((width, height), expected_tier) in cases {
            let layout = AppLayout::calculate(Rect::new(0, 0, width, height)).unwrap();
            assert_eq!(layout.tier, expected_tier);
            assert_non_empty(layout.search);
            assert_non_empty(layout.table);
            assert_non_empty(layout.details);
            assert_non_empty(layout.stats);
            assert_non_empty(layout.output);
            assert_non_empty(layout.help);
        }
    }

    fn assert_non_empty(area: Rect) {
        assert!(area.width > 0, "area width collapsed: {area:?}");
        assert!(area.height > 0, "area height collapsed: {area:?}");
    }
}
