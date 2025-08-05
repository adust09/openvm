use getset::WithSetters;
use openvm_stark_backend::p3_field::PrimeField32;
use p3_baby_bear::BabyBear;
use serde::{Deserialize, Serialize};

pub const DEFAULT_SEGMENT_CHECK_INSNS: u64 = 1000;

const DEFAULT_MAX_TRACE_HEIGHT: u32 = (1 << 23) - 10000;
pub const DEFAULT_MAX_CELLS: usize = 2_000_000_000; // 2B
const DEFAULT_MAX_INTERACTIONS: usize = BabyBear::ORDER_U32 as usize;

#[derive(derive_new::new, Clone, Debug, Serialize, Deserialize)]
pub struct Segment {
    pub instret_start: u64,
    pub num_insns: u64,
    pub trace_heights: Vec<u32>,
}

#[derive(Clone, Copy, Debug, WithSetters)]
pub struct SegmentationLimits {
    #[getset(set_with = "pub")]
    pub max_trace_height: u32,
    #[getset(set_with = "pub")]
    pub max_cells: usize,
    #[getset(set_with = "pub")]
    pub max_interactions: usize,
}

impl Default for SegmentationLimits {
    fn default() -> Self {
        Self {
            max_trace_height: DEFAULT_MAX_TRACE_HEIGHT,
            max_cells: DEFAULT_MAX_CELLS,
            max_interactions: DEFAULT_MAX_INTERACTIONS,
        }
    }
}

#[derive(Clone, Debug, WithSetters)]
pub struct SegmentationCtx {
    pub segments: Vec<Segment>,
    pub(crate) air_names: Vec<String>,
    widths: Vec<usize>,
    interactions: Vec<usize>,
    pub(crate) segmentation_limits: SegmentationLimits,
    pub instret_last_segment_check: u64,
    #[getset(set_with = "pub")]
    pub segment_check_insns: u64,
}

impl SegmentationCtx {
    pub fn new(
        air_names: Vec<String>,
        widths: Vec<usize>,
        interactions: Vec<usize>,
        segmentation_limits: SegmentationLimits,
    ) -> Self {
        assert_eq!(air_names.len(), widths.len());
        assert_eq!(air_names.len(), interactions.len());

        Self {
            segments: Vec::new(),
            air_names,
            widths,
            interactions,
            segmentation_limits,
            segment_check_insns: DEFAULT_SEGMENT_CHECK_INSNS,
            instret_last_segment_check: 0,
        }
    }

    pub fn new_with_default_segmentation_limits(
        air_names: Vec<String>,
        widths: Vec<usize>,
        interactions: Vec<usize>,
    ) -> Self {
        assert_eq!(air_names.len(), widths.len());
        assert_eq!(air_names.len(), interactions.len());

        Self {
            segments: Vec::new(),
            air_names,
            widths,
            interactions,
            segmentation_limits: SegmentationLimits::default(),
            segment_check_insns: DEFAULT_SEGMENT_CHECK_INSNS,
            instret_last_segment_check: 0,
        }
    }

    pub fn set_max_trace_height(&mut self, max_trace_height: u32) {
        self.segmentation_limits.max_trace_height = max_trace_height;
    }

    pub fn set_max_cells(&mut self, max_cells: usize) {
        self.segmentation_limits.max_cells = max_cells;
    }

    pub fn set_max_interactions(&mut self, max_interactions: usize) {
        self.segmentation_limits.max_interactions = max_interactions;
    }

    #[allow(dead_code)]
    pub fn print_segments(&self) {
        // Calculate dynamic widths based on actual content
        let max_air_name_width = self
            .air_names
            .iter()
            .map(|name| name.len())
            .max()
            .unwrap_or(8)
            .max(8); // minimum width for "Air Name"

        let max_height = self
            .segments
            .iter()
            .flat_map(|segment| segment.trace_heights.iter())
            .max()
            .unwrap_or(&0);
        let height_width = format!("{}", max_height).len().max(6); // minimum width for "Height"

        let max_cells = self
            .segments
            .iter()
            .flat_map(|segment| {
                segment
                    .trace_heights
                    .iter()
                    .enumerate()
                    .map(|(j, &height)| {
                        let width = self.widths.get(j).unwrap_or(&0);
                        height as usize * width
                    })
            })
            .max()
            .unwrap_or(0);
        let cells_width = format!("{}", max_cells).len().max(5); // minimum width for "Cells"

        for (i, segment) in self.segments.iter().enumerate() {
            println!("\nSegment #{}", i);
            println!("  Instret Start: {}", segment.instret_start);
            println!("  Num Insns:     {}", segment.num_insns);

            println!(
                "  | {:<width$} | {:>height_width$} | {:>cells_width$} |",
                "Air Name",
                "Height",
                "Cells",
                width = max_air_name_width,
                height_width = height_width,
                cells_width = cells_width
            );
            println!(
                "  |{:-<width$}|{:-<height_width$}|{:-<cells_width$}|",
                "",
                "",
                "",
                width = max_air_name_width + 2,
                height_width = height_width + 2,
                cells_width = cells_width + 2
            );

            for (j, height) in segment.trace_heights.iter().enumerate() {
                let air_name = self
                    .air_names
                    .get(j)
                    .map(|s| s.as_str())
                    .unwrap_or("Unknown");
                let width = self.widths.get(j).unwrap_or(&0);
                let cells = *height as usize * width;
                println!(
                    "  | {:<width$} | {:>height_width$} | {:>cells_width$} |",
                    air_name,
                    height,
                    cells,
                    width = max_air_name_width,
                    height_width = height_width,
                    cells_width = cells_width
                );
            }
        }
        println!();
    }

    /// Calculate the total cells used based on trace heights and widths
    #[inline(always)]
    fn calculate_total_cells(&self, trace_heights: &[u32]) -> usize {
        debug_assert_eq!(trace_heights.len(), self.widths.len());

        // SAFETY: Length equality is asserted during initialization
        let widths_slice = unsafe { self.widths.get_unchecked(..trace_heights.len()) };

        trace_heights
            .iter()
            .zip(widths_slice)
            .map(|(&height, &width)| height as usize * width)
            .sum()
    }

    /// Calculate the total interactions based on trace heights and interaction counts
    #[inline(always)]
    fn calculate_total_interactions(&self, trace_heights: &[u32]) -> usize {
        debug_assert_eq!(trace_heights.len(), self.interactions.len());

        // SAFETY: Length equality is asserted during initialization
        let interactions_slice = unsafe { self.interactions.get_unchecked(..trace_heights.len()) };

        trace_heights
            .iter()
            .zip(interactions_slice)
            // We add 1 for the zero messages from the padding rows
            .map(|(&height, &interactions)| (height + 1) as usize * interactions)
            .sum()
    }

    #[inline(always)]
    fn should_segment(
        &self,
        instret: u64,
        trace_heights: &[u32],
        is_trace_height_constant: &[bool],
    ) -> bool {
        debug_assert_eq!(trace_heights.len(), is_trace_height_constant.len());
        debug_assert_eq!(trace_heights.len(), self.air_names.len());

        let instret_start = self
            .segments
            .last()
            .map_or(0, |s| s.instret_start + s.num_insns);
        let num_insns = instret - instret_start;

        // Segment should contain at least one cycle
        if num_insns == 0 {
            return false;
        }

        for (i, (height, is_constant)) in trace_heights
            .iter()
            .zip(is_trace_height_constant.iter())
            .enumerate()
        {
            // Only segment if the height is not constant and exceeds the maximum height
            if !is_constant && *height > self.segmentation_limits.max_trace_height {
                let air_name = &self.air_names[i];
                tracing::info!(
                    "Segment {:2} | instret {:9} | chip {} ({}) height ({:8}) > max ({:8})",
                    self.segments.len(),
                    instret,
                    i,
                    air_name,
                    height,
                    self.segmentation_limits.max_trace_height
                );
                return true;
            }
        }

        let total_cells = self.calculate_total_cells(trace_heights);
        if total_cells > self.segmentation_limits.max_cells {
            tracing::info!(
                "Segment {:2} | instret {:9} | total cells ({:10}) > max ({:10})",
                self.segments.len(),
                instret,
                total_cells,
                self.segmentation_limits.max_cells
            );
            return true;
        }

        let total_interactions = self.calculate_total_interactions(trace_heights);
        if total_interactions > self.segmentation_limits.max_interactions {
            tracing::info!(
                "Segment {:2} | instret {:9} | total interactions ({:11}) > max ({:11})",
                self.segments.len(),
                instret,
                total_interactions,
                self.segmentation_limits.max_interactions
            );
            return true;
        }

        false
    }

    #[inline(always)]
    pub fn check_and_segment(
        &mut self,
        instret: u64,
        trace_heights: &[u32],
        is_trace_height_constant: &[bool],
    ) -> bool {
        let ret = self.should_segment(instret, trace_heights, is_trace_height_constant);
        if ret {
            self.segment(instret, trace_heights);
        }
        self.instret_last_segment_check = instret;

        ret
    }

    /// Try segment if there is at least one cycle
    #[inline(always)]
    pub fn segment(&mut self, instret: u64, trace_heights: &[u32]) {
        let instret_start = self
            .segments
            .last()
            .map_or(0, |s| s.instret_start + s.num_insns);
        let num_insns = instret - instret_start;

        debug_assert!(num_insns > 0, "Segment should contain at least one cycle");

        self.segments.push(Segment {
            instret_start,
            num_insns,
            trace_heights: trace_heights.to_vec(),
        });
    }
}
