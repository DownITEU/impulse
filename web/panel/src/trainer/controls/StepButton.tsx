import { hapticTap } from '@/shared/haptics';
import { cn } from '@/shared/lib/ui';
import { Icon } from '@/shared/ui/Icon';

export const STEPPER_SHELL_CLASS =
    'grid h-[38px] w-full items-stretch overflow-hidden rounded-[10px] border border-white/10 bg-white/5.5 shadow-[inset_0_1px_0_rgba(255,255,255,0.05)] backdrop-blur-xl';

type StepButtonProps = {
    icon: 'minus' | 'plus';
    border: 'left' | 'right';
    /** Required: the button renders an icon only, so it has no other accessible name. */
    label: string;
    disabled: boolean;
    onClick: () => void;
};

export const StepButton = ({ icon, border, label, disabled, onClick }: StepButtonProps) => {
    const handleClick = () => {
        hapticTap();
        onClick();
    };

    return (
        <button
            type="button"
            aria-label={label}
            disabled={disabled}
            className={cn(
                'flex items-center justify-center bg-white/2.5 text-(--deck-fg-2) transition-colors hover:bg-white/6 hover:text-(--deck-fg) disabled:cursor-not-allowed disabled:opacity-35 disabled:hover:bg-white/2.5 disabled:hover:text-(--deck-fg-2)',
                border === 'right' ? 'border-r border-white/10' : 'border-l border-white/10',
            )}
            onClick={handleClick}
        >
            <Icon className="size-4" name={icon} stroke={2} />
        </button>
    );
};
