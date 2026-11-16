import type { Section } from '@/api';
import type { Ready } from '@/session/state';
import { spot } from '@/shared/lib/ui';

/** A section as the app sent it, with only the cheats the query leaves. */
export type Shown = { section: Section; cheats: number[] };

/** The sections the cheat names match `query` in: their order, their grouping, their wording. */
export function shownSections(ready: Ready, query: string): Shown[] {
    const matches = (cheat: number) => spot(ready.cheats[cheat]?.name ?? '', query) !== null;
    return ready.sections
        .map((section) => ({ section, cheats: section.cheats.filter(matches) }))
        .filter((shown) => shown.cheats.length > 0);
}

/** How many cheats the sections show: a cheat in two sections counts once. */
export function countShown(shown: Shown[]): number {
    return new Set(shown.flatMap((one) => one.cheats)).size;
}

/** The toggles among `cheats` and how many of them are on. */
export function countToggles(
    ready: Ready,
    cheats: number[],
    values: number[] | null,
): { toggles: number; on: number } {
    let toggles = 0;
    let on = 0;
    for (const cheat of cheats) {
        if (ready.cheats[cheat]?.control.kind !== 'toggle') continue;
        toggles += 1;
        if (values?.[cheat]) on += 1;
    }
    return { toggles, on };
}
