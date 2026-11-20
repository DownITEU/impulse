import { useSliderGesture } from './use-slider-gesture';

type SliderTrackProps = {
    min: number;
    max: number;
    step: number;
    value: number;
    label: string;
    disabled: boolean;
    onMove: (value: number) => void;
    /** The drag ended, or the keyboard set the value. */
    onEnd: (value: number) => void;
};

export const SliderTrack = ({
    min,
    max,
    step,
    value,
    label,
    disabled,
    onMove,
    onEnd,
}: SliderTrackProps) => {
    const pct = max === min ? 0 : Math.max(0, Math.min(100, ((value - min) / (max - min)) * 100));
    const gesture = useSliderGesture(value, { min, max, step }, disabled, onMove, onEnd);

    return (
        <div className="relative flex h-9 w-full items-center touch-pan-y" {...gesture}>
            <div className="pointer-events-none absolute inset-x-0 h-1 overflow-hidden rounded-full bg-white/6">
                <div
                    className="h-full rounded-full bg-[linear-gradient(90deg,color-mix(in_oklab,var(--deck-accent)_60%,transparent),var(--deck-accent))]"
                    style={{ width: `${pct}%` }}
                />
            </div>
            <input
                type="range"
                aria-label={label}
                min={min}
                max={max}
                step={step}
                value={value}
                disabled={disabled}
                className="remote-range pointer-events-none w-full"
                onInput={(event) => onEnd(Number(event.currentTarget.value))}
            />
        </div>
    );
};
