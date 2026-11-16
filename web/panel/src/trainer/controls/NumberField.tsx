import { useRef, useState } from 'react';

import { useLanguage } from '@/i18n/texts';

import { formatInput, parseInput } from './format-number';

type NumberFieldProps = {
    value: number;
    digits?: number;
    /** Required: the field has no visible label. */
    label: string;
    disabled: boolean;
    className: string;
    /** The edit ended (Enter or blur) with a number in the app's language. */
    onCommit: (value: number) => void;
    /** Each keystroke: the number typed so far, `null` while it is not one. */
    onEdit?: (value: number | null) => void;
};

/** Edit text in the app's language: shown formatted, typed raw, read back with its decimal mark. */
export const NumberField = ({
    value,
    digits,
    label,
    disabled,
    className,
    onCommit,
    onEdit,
}: NumberFieldProps) => {
    const tag = useLanguage();
    const [typed, setTyped] = useState<string | null>(null);
    // The text an Enter has just read must not be read again by the blur that follows.
    const text = useRef<string | null>(null);

    const edit = (next: string | null) => {
        text.current = next;
        setTyped(next);
    };
    const end = () => {
        const written = text.current;
        edit(null);
        const parsed = written === null ? null : parseInput(tag, written);
        if (parsed !== null) onCommit(parsed);
    };

    return (
        <input
            type="text"
            inputMode="decimal"
            aria-label={label}
            disabled={disabled}
            value={typed ?? formatInput(tag, value, digits)}
            className={className}
            onInput={(event) => {
                edit(event.currentTarget.value);
                onEdit?.(parseInput(tag, event.currentTarget.value));
            }}
            onBlur={end}
            onKeyDown={(event) => {
                if (event.key === 'Enter') end();
                if (event.key === 'Escape') {
                    event.stopPropagation();
                    edit(null);
                }
            }}
        />
    );
};
