use std::{
  fmt::{self, Debug},
  sync::Arc,
};

use rspack_cacheable::cacheable;
use rspack_location::{DependencyLocation, RealDependencyLocation, SourcePosition};

/// Represents a range in a dependency, typically used for tracking the span of code in a source file.
/// It stores the start and end positions (as offsets) of the range, typically using base-0 indexing.
#[cacheable]
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct DependencyRange {
  pub end: u32,
  pub start: u32,
}

impl From<(u32, u32)> for DependencyRange {
  fn from(range: (u32, u32)) -> Self {
    Self {
      start: range.0,
      end: range.1,
    }
  }
}

impl From<swc_core::common::Span> for DependencyRange {
  fn from(span: swc_core::common::Span) -> Self {
    Self {
      start: span.lo.0.saturating_sub(1),
      end: span.hi.0.saturating_sub(1),
    }
  }
}

impl DependencyRange {
  pub fn new(start: u32, end: u32) -> Self {
    DependencyRange { end, start }
  }

  /// Converts the `DependencyRange` into a `DependencyLocation`.
  /// The `source` parameter is an optional source map used to resolve the exact position in the source file.
  pub fn to_loc<T: AsLoc>(&self, source: Option<T>) -> Option<DependencyLocation> {
    source
      .and_then(|s| s.as_loc().look_up_range_pos(self.start, self.end))
      .map(|(start, end)| {
        DependencyLocation::Real(if start.line == end.line && start.column == end.column {
          RealDependencyLocation::new(start, None)
        } else {
          RealDependencyLocation::new(start, Some(end))
        })
      })
  }
}

/// Trait representing a source map that can resolve the positions of code ranges to source file positions.
pub trait SourceLocation: Send + Sync {
  fn look_up_range_pos(&self, start: u32, end: u32) -> Option<(SourcePosition, SourcePosition)>;
}

impl Debug for dyn SourceLocation {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("SourceMap").finish()
  }
}

impl SourceLocation for swc_core::common::SourceMap {
  fn look_up_range_pos(&self, start: u32, end: u32) -> Option<(SourcePosition, SourcePosition)> {
    let lo = self.lookup_char_pos(swc_core::common::BytePos(start + 1));
    let hi = self.lookup_char_pos(swc_core::common::BytePos(end + 1));

    Some((
      SourcePosition {
        line: lo.line,
        column: lo.col_display,
      },
      SourcePosition {
        line: hi.line,
        column: hi.col_display,
      },
    ))
  }
}

impl SourceLocation for &str {
  fn look_up_range_pos(&self, start: u32, end: u32) -> Option<(SourcePosition, SourcePosition)> {
    let r = ropey::Rope::from_str(self);
    let start_char_offset = r.try_byte_to_char(start as usize).ok()?;
    let end_char_offset = r.try_byte_to_char(end as usize).ok()?;

    let start_line = r.char_to_line(start_char_offset);
    let start_column = start_char_offset - r.line_to_char(start_line);
    let end_line = r.char_to_line(end_char_offset);
    let end_column = end_char_offset - r.line_to_char(end_line);

    Some((
      SourcePosition {
        line: start_line + 1,
        column: start_column,
      },
      SourcePosition {
        line: end_line + 1,
        column: end_column,
      },
    ))
  }
}

/// Type alias for a shared reference to a `SourceLocation` trait object, typically used for source maps.
pub type SharedSourceMap = Arc<dyn SourceLocation>;

pub trait AsLoc {
  fn as_loc(&self) -> &dyn SourceLocation;
}

impl AsLoc for &Arc<dyn SourceLocation> {
  #[inline]
  fn as_loc(&self) -> &dyn SourceLocation {
    self.as_ref()
  }
}

impl AsLoc for &str {
  #[inline]
  fn as_loc(&self) -> &dyn SourceLocation {
    let loc: &dyn SourceLocation = self;
    loc
  }
}
