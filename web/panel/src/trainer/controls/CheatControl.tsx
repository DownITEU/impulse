import type { CheatRow } from '@/api';

import { ActionButton } from './ActionButton';
import { NumberControl } from './NumberControl';
import { SelectionControl } from './SelectionControl';
import { SliderControl } from './SliderControl';
import { ToggleControl } from './ToggleControl';

type CheatControlProps = {
    row: CheatRow;
    value: number | undefined;
    disabled: boolean;
    onSet: (value: number) => void;
    onPress: () => void;
};

export const CheatControl = ({
    row: { name, control },
    value,
    disabled,
    onSet,
    onPress,
}: CheatControlProps) => {
    switch (control.kind) {
        case 'toggle':
            return <ToggleControl name={name} value={value} disabled={disabled} onSet={onSet} />;
        case 'button':
            return <ActionButton disabled={disabled} onPress={onPress} />;
        case 'number':
            return (
                <NumberControl
                    name={name}
                    control={control}
                    value={value}
                    disabled={disabled}
                    onSet={onSet}
                />
            );
        case 'slider':
            return (
                <SliderControl
                    name={name}
                    control={control}
                    value={value}
                    disabled={disabled}
                    onSet={onSet}
                />
            );
        case 'selection':
            return (
                <SelectionControl
                    name={name}
                    options={control.options}
                    value={value}
                    disabled={disabled}
                    onSet={onSet}
                />
            );
        case 'none':
            return null;
    }
};

export const isStacked = ({ control }: CheatRow): boolean =>
    control.kind !== 'toggle' && control.kind !== 'none';
