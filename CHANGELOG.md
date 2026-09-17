# Changelog

## [1.1.1] - 2026-09-17

The popup no longer overflows a typical top bar.

### Fixed
- Clamp the card to the available screen size instead of a raw 420×580 window that ran off the display
- Put a long budget name on its own row so the web / refresh / settings buttons stay on the card
- Scroll buckets, income, spending, and settings inside the card instead of growing it
- Keep the Income tab's 6-month labels on screen without scrolling

### Changed
- Tab labels are now **Buckets / Income / Spending**
- Desired card size is 480×600, still clamped when the screen is smaller

## [1.1.0] - 2026-08-26

Decouple background sync into a headless service and a lightweight bar widget.

## [1.0.0] - 2026-08-25

Initial release of YNAB Pulse.
