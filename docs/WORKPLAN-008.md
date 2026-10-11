# Preload regression work plan
1. Reproduce the reported placeholder using full UI frames and a 2048 MB budget.
2. Distinguish cache disposal, unfinished decode and budget eviction with targeted assertions.
3. Fix the proven cause and verify visible/offscreen transitions, zoom and budget changes.
4. Run regressions/lint/build; document limits and publish the Windows fix.
Reproduction complete: before the fix, the 2048 MB real-decoder/UI-frame regression failed at offscreen texture retention (55 other tests passed).
Fix complete: shared tiers and retained larger-cache accounting. The same regression now passes. Additional real offscreen decoding covers normal/vertical/horizontal views, pan into view, fractional zoom, zoom-out/back and zero-budget eviction.
Validation: 57 automated tests pass; two optional load benchmarks were not run. Formatting, strict lint and Windows release build passed. Package contains only executable, bilingual guides and third-party notices; publication targets the existing public repository.
