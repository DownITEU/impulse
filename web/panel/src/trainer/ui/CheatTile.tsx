import { memo, type PointerEvent as ReactPointerEvent, useEffect, useRef, useState } from 'react';

import type { CheatRow } from '@/api';
import { useT } from '@/i18n/texts';
import { send } from '@/session/store';
import { cn } from '@/shared/lib/ui';
import { Icon } from '@/shared/ui/Icon';
import { IconButton } from '@/shared/ui/IconButton';

import { CheatControl, isStacked } from '../controls/CheatControl';

type CheatTileProps = {
    sheet: number;
    cheat: number;
    row: CheatRow;
    value: number | undefined;
    disabled: boolean;
    first: boolean;
};

const SWIPE_REVEAL = 80;
const SWIPE_TRIGGER = 56;
const SWIPE_DEAD_ZONE = 8;
const SWIPE_ANIMATION_MS = 200;

const CheatTileBase = ({ sheet, cheat, row, value, disabled, first }: CheatTileProps) => {
    const t = useT();
    const [offset, setOffset] = useState(0);
    const [animating, setAnimating] = useState(false);
    const [armed, setArmed] = useState(false);
    const dragRef = useRef<{
        id: number;
        startX: number;
        startY: number;
        locked: boolean | null;
    } | null>(null);
    const settleTimerRef = useRef<number | null>(null);
    const { pinned } = row;
    const pin = () => send({ type: 'pin', sheet, cheat });

    useEffect(() => {
        setOffset(0);
        setArmed(false);
    }, [pinned]);

    // The tile can unmount while the timer is pending.
    useEffect(
        () => () => {
            if (settleTimerRef.current !== null) window.clearTimeout(settleTimerRef.current);
        },
        [],
    );

    const settle = (target: number) => {
        setAnimating(true);
        setOffset(target);
        if (settleTimerRef.current !== null) window.clearTimeout(settleTimerRef.current);
        settleTimerRef.current = window.setTimeout(() => {
            settleTimerRef.current = null;
            setAnimating(false);
        }, SWIPE_ANIMATION_MS);
    };

    const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
        if (event.pointerType === 'mouse' && event.button !== 0) return;
        if (isInteractiveTarget(event.target)) return;
        dragRef.current = {
            id: event.pointerId,
            startX: event.clientX,
            startY: event.clientY,
            locked: null,
        };
        setAnimating(false);
    };

    const handlePointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
        const drag = dragRef.current;
        if (!drag || drag.id !== event.pointerId) return;
        const dx = event.clientX - drag.startX;
        const dy = event.clientY - drag.startY;
        if (drag.locked === null) {
            if (Math.abs(dx) < SWIPE_DEAD_ZONE && Math.abs(dy) < SWIPE_DEAD_ZONE) return;
            drag.locked = Math.abs(dx) > Math.abs(dy) && dx < 0;
            if (!drag.locked) {
                dragRef.current = null;
                return;
            }
            event.currentTarget.setPointerCapture(event.pointerId);
        }
        const next = Math.min(Math.max(dx, -SWIPE_REVEAL * 1.2), 0);
        setOffset(next);
        setArmed(-next >= SWIPE_TRIGGER);
    };

    // Only a lifted finger pins; a cancelled swipe just slides back.
    const handlePointerEnd = (event: ReactPointerEvent<HTMLDivElement>, lifted: boolean) => {
        const drag = dragRef.current;
        if (!drag || drag.id !== event.pointerId) return;
        dragRef.current = null;
        if (drag.locked !== true) return;
        if (event.currentTarget.hasPointerCapture(event.pointerId)) {
            event.currentTarget.releasePointerCapture(event.pointerId);
        }
        const triggered = lifted && -offset >= SWIPE_TRIGGER;
        settle(0);
        setArmed(false);
        if (triggered) pin();
    };

    const stacked = isStacked(row);
    const pinButton = (
        <IconButton
            className={cn(stacked && 'ml-auto', 'border-transparent bg-transparent')}
            icon={pinned ? 'pin-off' : 'pin'}
            label={
                pinned ? t('cheat-unpin', { name: row.name }) : t('cheat-pin', { name: row.name })
            }
            active={pinned}
            size="sm"
            shrink
            onClick={pin}
        />
    );
    const control = (
        <CheatControl
            row={row}
            value={value}
            disabled={disabled}
            onSet={(next) => send({ type: 'set', sheet, cheat, value: next })}
            onPress={() => send({ type: 'press', sheet, cheat })}
        />
    );

    return (
        <div className={cn('relative overflow-hidden', first ? '' : 'border-t border-white/6')}>
            {offset < 0 ? <PinReveal pinned={pinned} armed={armed} /> : null}
            <div
                className={cn('relative', animating ? 'transition-transform ease-out' : '')}
                style={{
                    transform: `translate3d(${offset}px, 0, 0)`,
                    touchAction: 'pan-y',
                    transitionDuration: animating ? `${SWIPE_ANIMATION_MS}ms` : undefined,
                }}
                onPointerCancel={(event) => handlePointerEnd(event, false)}
                onPointerDown={handlePointerDown}
                onPointerMove={handlePointerMove}
                onPointerUp={(event) => handlePointerEnd(event, true)}
            >
                <div className="flex flex-col gap-2 px-3.5 py-3">
                    <div
                        className={cn(
                            'flex gap-3',
                            stacked ? 'flex-col items-stretch' : 'items-center justify-between',
                        )}
                    >
                        <div className="min-w-0 flex-1">
                            <div className="flex items-center gap-2">
                                <h4 className="truncate text-[13.5px] font-medium leading-tight text-(--deck-fg)">
                                    {row.name}
                                </h4>
                                {/* The swipe is pointer-only; this is the keyboard and screen-reader path. */}
                                {stacked ? pinButton : null}
                            </div>
                            {row.description ? (
                                <p className="mt-1 line-clamp-2 text-[11.5px] leading-snug text-(--deck-fg-3)">
                                    {row.description}
                                </p>
                            ) : null}
                        </div>
                        {stacked ? (
                            control
                        ) : (
                            <div className="flex shrink-0 items-center gap-2">
                                {pinButton}
                                {control}
                            </div>
                        )}
                    </div>
                    {row.instructions ? (
                        <div className="flex gap-2 rounded-[8px] border border-amber-300/25 bg-amber-400/10 px-2.5 py-2 text-[11.5px] leading-5 text-amber-200">
                            <Icon className="mt-0.5 size-3.5 shrink-0" name="alert" />
                            <span>{row.instructions}</span>
                        </div>
                    ) : null}
                </div>
            </div>
        </div>
    );
};

/** Renders again only for its own value or row: a values view leaves the other tiles alone. */
export const CheatTile = memo(CheatTileBase);

const PinReveal = ({ pinned, armed }: { pinned: boolean; armed: boolean }) => (
    <div className="pointer-events-none absolute inset-y-0 right-0 flex items-center justify-center px-4">
        <span
            className={cn(
                'flex size-9 items-center justify-center rounded-full border transition-all duration-150',
                armed
                    ? 'scale-110 border-(--deck-accent) bg-[color-mix(in_oklab,var(--deck-accent)_28%,transparent)] text-(--deck-accent) shadow-[0_0_0_4px_color-mix(in_oklab,var(--deck-accent)_18%,transparent)]'
                    : 'border-white/10 bg-white/6 text-(--deck-fg-3)',
            )}
        >
            <Icon className="size-4" name={pinned ? 'pin-off' : 'pin'} />
        </span>
    </div>
);

function isInteractiveTarget(target: EventTarget | null): boolean {
    return (
        target instanceof Element &&
        target.closest('input, button, select, textarea, a, [role="slider"], [role="button"]') !==
            null
    );
}
