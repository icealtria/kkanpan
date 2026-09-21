// PocketJS platform contract patch for Kindle Paperwhite 3.
//
// Apply this to contracts/spec/platforms.ts in the pocketjs repo.
// Add `kindle` to the TargetRegistry type and POCKET_TARGETS object.

// ============================================================
// 1. Add to the type declaration (after pocketbook entry):
// ============================================================

// readonly kindle: TargetProfile<PocketCapabilityId>;

// ============================================================
// 2. Add to POCKET_TARGETS object:
// ============================================================

/*
kindle: {
  hostAbi: 6,
  platform: "kindle",
  form: "takeover",
  display: {
    physicalViewport: [1072, 1448],
    // NOTE: Logical viewport must be ≤511/axis for the 9-bit touch wire format.
    // 724 > 511, so we use 511 as the logical height with letterboxing.
    // Alternative: use density 3 (358×483) for exact fit but coarser layout.
    logicalViewports: [[536, 511]],
    presentations: ["native"],
    rasterDensity: 2,
  },
  capabilities: [
    "input.buttons",
    "input.touch",
    "text.glyphs.baked",
  ],
},
*/
