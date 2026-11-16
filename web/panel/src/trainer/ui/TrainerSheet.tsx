import { useState } from 'react';
import { useT } from '@/i18n/texts';
import type { Trainer } from '@/session/state';
import { PlaceholderState } from '@/shared/ui/PlaceholderState';
import { SearchInput } from '@/shared/ui/SearchInput';

import { countShown, shownSections } from '../model/sections';
import { QuickActions } from './QuickActions';
import { Section } from './Section';

type TrainerSheetProps = {
    trainer: Trainer;
    values: number[] | null;
    online: boolean;
    live: boolean;
};

/** The open game's cheats, or why they are not there. */
export const TrainerSheet = ({
    trainer: { sheet, trainer },
    values,
    online,
    live,
}: TrainerSheetProps) => {
    const t = useT();
    const [query, setQuery] = useState('');

    switch (trainer.state) {
        case 'none':
            return (
                <PlaceholderState
                    icon="gamepad-variant-outline"
                    title={t('sheet-none')}
                    sub={t('sheet-none-body')}
                />
            );
        case 'pending':
            return <PlaceholderState icon="loader" title={t('sheet-pending')} />;
        case 'failed':
            return <PlaceholderState icon="alert" title={t('sheet-failed')} sub={trainer.why} />;
    }

    const shown = shownSections(trainer, query);
    const matches = countShown(shown);

    return (
        <>
            <QuickActions
                sheet={sheet}
                collections={trainer.collections}
                online={online}
                live={live}
            />
            <div className="sticky top-0 z-10 -mx-3.5 mb-2.5 px-3.5 py-0.5">
                <SearchInput value={query} placeholder={t('sheet-search')} onChange={setQuery} />
            </div>
            {online && !live ? (
                <p className="mb-2.5 text-center text-[11.5px] text-(--deck-fg-3)">
                    {t('sheet-start-first')}
                </p>
            ) : null}
            {shown.map((one, index) => (
                <Section
                    key={`${one.section.pinned}:${one.section.name}`}
                    sheet={sheet}
                    ready={trainer}
                    shown={one}
                    values={values}
                    live={live}
                    openByDefault={index < 2}
                    forceOpen={query !== ''}
                />
            ))}
            {query && matches === 0 ? (
                <p className="px-8 py-8 text-center text-[13px] text-(--deck-fg-4)">
                    {t('sheet-no-match', { query })}
                </p>
            ) : null}
            <div className="mt-4 text-center font-mono text-[10px] uppercase tracking-[0.08em] text-(--deck-fg-4)">
                {query
                    ? t('sheet-matches', { count: matches })
                    : t('sheet-end', { count: trainer.cheats.length })}
            </div>
        </>
    );
};
