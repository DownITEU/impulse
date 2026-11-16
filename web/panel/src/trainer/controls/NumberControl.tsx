import { useEffect, useState } from 'react';

import type { Control } from '@/api';
import { useT } from '@/i18n/texts';
import { hapticTap } from '@/shared/haptics';
import { cn } from '@/shared/lib/ui';

import { NumberField } from './NumberField';
import { STEPPER_SHELL_CLASS, StepButton } from './StepButton';
import { nudge } from './step';
import { useDraft } from './use-draft';

const STEP_GRID = 'grid-cols-[48px_minmax(0,1fr)_48px]';

type NumberControlProps = {
    name: string;
    control: Extract<Control, { kind: 'number' }>;
    value: number | undefined;
    disabled: boolean;
    onSet: (value: number) => void;
};

export const NumberControl = ({ name, control, value, disabled, onSet }: NumberControlProps) => {
    const t = useT();
    const { min, max, step, apply } = control;
    const [shown, show] = useDraft(value);
    // With `apply` the number waits here until the Apply button sends it.
    const [waiting, setWaiting] = useState<number | null>(null);
    const current = waiting ?? shown ?? 0;

    // A staged number belongs to the value it was made over, while the control acts.
    useEffect(() => setWaiting(null), [value, disabled]);

    const bound = (next: number) =>
        Math.min(max ?? Number.POSITIVE_INFINITY, Math.max(min ?? Number.NEGATIVE_INFINITY, next));
    const settle = (next: number) => {
        if (apply) {
            setWaiting(bound(next));
        } else {
            show(bound(next));
            onSet(bound(next));
        }
    };
    const send = () => {
        if (waiting === null) return;
        hapticTap();
        show(waiting);
        onSet(waiting);
        setWaiting(null);
    };

    return (
        <div className="w-full space-y-1.5">
            <div className={cn(STEPPER_SHELL_CLASS, STEP_GRID)}>
                <StepButton
                    border="right"
                    disabled={disabled}
                    icon="minus"
                    label={t('control-decrease', { name })}
                    onClick={() => settle(nudge(current, -step))}
                />
                <NumberField
                    label={name}
                    value={current}
                    disabled={disabled}
                    className="min-w-0 bg-transparent px-2 text-center font-mono text-[15px] font-semibold tabular-nums text-(--deck-fg) outline-none disabled:opacity-50"
                    onCommit={settle}
                    onEdit={
                        apply ? (next) => setWaiting(next === null ? null : bound(next)) : undefined
                    }
                />
                <StepButton
                    border="left"
                    disabled={disabled}
                    icon="plus"
                    label={t('control-increase', { name })}
                    onClick={() => settle(nudge(current, step))}
                />
            </div>
            {apply ? (
                <button
                    type="button"
                    disabled={disabled || waiting === null}
                    className="h-9 w-full rounded-[10px] bg-(--deck-accent) text-[12px] font-bold text-black disabled:cursor-not-allowed disabled:opacity-40"
                    onClick={send}
                >
                    {t('control-apply')}
                </button>
            ) : null}
        </div>
    );
};
