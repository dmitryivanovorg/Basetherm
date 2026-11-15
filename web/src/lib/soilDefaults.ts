// Default soil properties of a class at a saturation, from the solver's material model. They only
// fill the property inputs; the user may overwrite any of them.
import { soilClient } from '../api/soil';
import { soilFilled, type SoilForm } from './form';

/** Fallbacks for a custom soil without values. */
const CUSTOM = { k: 1.5, cv: 2.5e6 };

/** Overwrite the properties of `s` with the defaults of its class (a custom soil keeps its values). */
export async function applyClassDefaults(s: SoilForm): Promise<void> {
  if (s.preset === 'custom') {
    s.k ??= CUSTOM.k;
    s.cv ??= CUSTOM.cv;
    s.kFrozen ??= s.k;
    s.cvFrozen ??= s.cv;
    s.latent ??= 0;
    return;
  }
  if (!(s.saturation >= 0 && s.saturation <= 1)) return;
  const { preset, saturation } = s;
  const p = await soilClient.properties({ preset, saturation, freezing: true });
  if (s.preset !== preset || s.saturation !== saturation) return; // edited meanwhile; that edit refills
  s.k = p.k;
  s.kFrozen = p.kFrozen;
  s.cv = p.cv;
  s.cvFrozen = p.cvFrozen;
  s.latent = p.latent;
}

/** Fill the layers that have no properties yet (new form, older case file). */
export function fillMissing(soil: SoilForm[]) {
  for (const s of soil) if (!soilFilled(s)) applyClassDefaults(s).catch(() => {});
}
