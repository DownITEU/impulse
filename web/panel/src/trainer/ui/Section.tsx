import { useEffect, useId, useState } from 'react';

import { useT } from '@/i18n/texts';
import type { Ready } from '@/session/state';
import { cn } from '@/shared/lib/ui';
import { Icon } from '@/shared/ui/Icon';

import { countToggles, type Shown } from '../model/sections';
import { CheatTile } from './CheatTile';

type SectionProps = {
    sheet: number;
    ready: Ready;
    shown: Shown;
    values: number[] | null;
    live: boolean;
    openByDefault: boolean;
    /** A search is on: every section with a match opens. */
    forceOpen: boolean;
};

export const Section = ({
    sheet,
    ready,
    shown,
    values,
    live,
    openByDefault,
    forceOpen,
}: SectionProps) => {
    const t = useT();
    const [open, setOpen] = useState(openByDefault);
    const panel = useId();
    const { cheats, section } = shown;
    const { toggles, on } = countToggles(ready, cheats, values);
    const count = cheats.length;

    useEffect(() => {
        if (forceOpen) setOpen(true);
    }, [forceOpen]);

    return (
        <section className="mb-2.5 overflow-hidden rounded-[14px] border border-white/10 bg-white/3.5 shadow-[inset_0_1px_0_rgba(255,255,255,.05)] backdrop-blur-2xl">
            <button
                type="button"
                aria-expanded={open}
                aria-controls={panel}
                className="flex w-full items-center gap-2.5 px-3.5 py-3 text-left text-(--deck-fg)"
                onClick={() => setOpen((current) => !current)}
            >
                <span className="flex size-7.5 shrink-0 items-center justify-center rounded-[8px] border border-[color-mix(in_oklab,var(--deck-accent)_22%,transparent)] bg-white/4 text-(--deck-accent)">
                    <Icon className="size-3.75" name={section.pinned ? 'pin' : 'sparkles'} />
                </span>
                <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm font-semibold">{section.name}</span>
                    <span className="mt-0.5 block font-mono text-[10.5px] text-(--deck-fg-4)">
                        {toggles > 0
                            ? t('section-mods-on', { count, on, toggles })
                            : t('section-mods', { count })}
                    </span>
                </span>
                {on > 0 ? (
                    <span className="inline-flex h-4.5 min-w-4.5 shrink-0 items-center justify-center rounded-full bg-(--deck-accent) px-1.5 text-center font-mono text-[10px] font-bold leading-none tabular-nums text-black">
                        {on}
                    </span>
                ) : null}
                <Icon
                    className={cn(
                        'size-4 text-(--deck-fg-3) transition-transform',
                        open ? 'rotate-0' : '-rotate-90',
                    )}
                    name="chevron-down"
                />
            </button>
            <div
                id={panel}
                className={cn(
                    'grid transition-[grid-template-rows] duration-300',
                    open ? 'grid-rows-[1fr]' : 'grid-rows-[0fr]',
                )}
            >
                <div className="overflow-hidden" inert={open ? undefined : true}>
                    {cheats.map((cheat, index) => (
                        <CheatTile
                            key={cheat}
                            sheet={sheet}
                            cheat={cheat}
                            row={ready.cheats[cheat]}
                            value={values?.[cheat]}
                            disabled={!live}
                            first={index === 0}
                        />
                    ))}
                </div>
            </div>
        </section>
    );
};
