import { useT } from '@/i18n/texts';
import { hapticTap } from '@/shared/haptics';
import { Icon } from '@/shared/ui/Icon';

type ActionButtonProps = {
    disabled: boolean;
    onPress: () => void;
};

export const ActionButton = ({ disabled, onPress }: ActionButtonProps) => {
    const t = useT();
    const handleClick = () => {
        hapticTap();
        onPress();
    };

    return (
        <button
            type="button"
            disabled={disabled}
            className="inline-flex h-10 w-full items-center justify-center gap-2 rounded-[10px] bg-(--deck-accent) px-3 text-[13px] font-semibold text-black shadow-[0_8px_24px_-8px_color-mix(in_oklab,var(--deck-accent)_50%,transparent)] disabled:cursor-not-allowed disabled:opacity-50"
            onClick={handleClick}
        >
            <Icon className="size-3.5" name="bolt" stroke={2} />
            {t('control-press')}
        </button>
    );
};
