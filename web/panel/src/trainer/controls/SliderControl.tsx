import type { Control } from '@/api';
import { useT } from '@/i18n/texts';

import { NumberField } from './NumberField';
import { SliderTrack } from './SliderTrack';
import { clampToPrecision, decimalPlaces } from './step';
import { useDraft } from './use-draft';

type SliderControlProps = {
    name: string;
    control: Extract<Control, { kind: 'slider' }>;
    value: number | undefined;
    disabled: boolean;
    onSet: (value: number) => void;
};

export const SliderControl = ({ name, control, value, disabled, onSet }: SliderControlProps) => {
    const t = useT();
    const { min, max, step } = control;
    const [shown, show] = useDraft(value);
    const current = shown ?? min;
    const digits = Math.max(decimalPlaces(min), decimalPlaces(max), decimalPlaces(step));

    // The server coalesces a burst of sets, so every move goes out.
    const move = (next: number) => {
        show(next, true);
        onSet(next);
    };
    // A drag the run outlived ends without a set.
    const end = (next: number) => {
        show(next);
        if (!disabled) onSet(next);
    };

    return (
        <div className="flex w-full items-center gap-3">
            <SliderTrack
                disabled={disabled}
                label={name}
                max={max}
                min={min}
                step={step}
                value={current}
                onMove={move}
                onEnd={end}
            />
            <div className="relative flex h-9 w-24 shrink-0 items-center rounded-lg border border-white/10 bg-white/5 px-2 font-mono text-sm text-(--deck-accent)">
                <NumberField
                    label={t('control-value', { name })}
                    value={current}
                    digits={digits}
                    disabled={disabled}
                    className="min-w-0 w-full bg-transparent text-center tabular-nums outline-none disabled:opacity-50"
                    onCommit={(next) => end(clampToPrecision(next, min, max, step))}
                />
            </div>
        </div>
    );
};
