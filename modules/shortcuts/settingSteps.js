/**
 * Step amounts for nudging develop settings via ArrowUp / ArrowDown.
 * Normal step vs larger shiftStep when holding Shift.
 * @type {Record<string, { step: number, shiftStep: number }>}
 */
export const SETTING_STEPS = {
  exposure_ev: { step: 0.05, shiftStep: 0.25 },
  print_exposure_ev: { step: 0.05, shiftStep: 0.25 },
  temperature: { step: 50, shiftStep: 250 },
  tint: { step: 1, shiftStep: 5 },
  y_shift: { step: 1, shiftStep: 5 },
  m_shift: { step: 1, shiftStep: 5 },
  c_shift: { step: 1, shiftStep: 5 },
  contrast: { step: 1, shiftStep: 5 },
  highlights: { step: 1, shiftStep: 5 },
  shadows: { step: 1, shiftStep: 5 },
  whites: { step: 1, shiftStep: 5 },
  midtones: { step: 1, shiftStep: 5 },
  saturation: { step: 1, shiftStep: 5 },
  vibrance: { step: 1, shiftStep: 5 },
  grain: { step: 0.1, shiftStep: 0.5 },
  halation: { step: 0.1, shiftStep: 0.5 },
  diffusion: { step: 0.1, shiftStep: 0.5 },
  sharpen: { step: 0.1, shiftStep: 0.5 },
};
