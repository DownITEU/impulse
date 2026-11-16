import { type PointerEvent, useRef } from 'react';

import { clampToStep } from './step';

const DRAG_THRESHOLD = 8;

type SliderGesture = {
    pointerId: number;
    x: number;
    y: number;
    value: number;
    dragging: boolean;
    /** The value of the last move. */
    last: number | null;
};

type Span = { min: number; max: number; step: number };

export function useSliderGesture(
    value: number,
    { min, max, step }: Span,
    disabled: boolean,
    onMove: (value: number) => void,
    onEnd: (value: number) => void,
) {
    const gesture = useRef<SliderGesture | null>(null);

    const move = (event: PointerEvent<HTMLDivElement>) => {
        const start = gesture.current;
        if (!start || start.pointerId !== event.pointerId || disabled) return;
        const dx = event.clientX - start.x;
        const dy = event.clientY - start.y;
        if (!start.dragging) {
            if (Math.max(Math.abs(dx), Math.abs(dy)) < DRAG_THRESHOLD) return;
            if (Math.abs(dy) >= Math.abs(dx)) {
                gesture.current = null;
                return;
            }
            start.dragging = true;
        }
        const width = event.currentTarget.getBoundingClientRect().width;
        if (width <= 0) return;
        start.last = clampToStep(start.value + (dx / width) * (max - min), min, max, step);
        onMove(start.last);
    };

    // Every drag ends through `onEnd`, which releases the control's held draft.
    const release = () => {
        const start = gesture.current;
        gesture.current = null;
        if (start?.last != null) onEnd(start.last);
    };

    return {
        onPointerDown(event: PointerEvent<HTMLDivElement>) {
            if (disabled || event.button > 0 || gesture.current) return;
            // The tile swipe gesture must not steal the slider drag.
            event.stopPropagation();
            gesture.current = {
                pointerId: event.pointerId,
                x: event.clientX,
                y: event.clientY,
                value,
                dragging: false,
                last: null,
            };
            event.currentTarget.setPointerCapture?.(event.pointerId);
        },
        onPointerMove: move,
        onPointerUp(event: PointerEvent<HTMLDivElement>) {
            if (gesture.current?.pointerId !== event.pointerId) return;
            move(event);
            release();
        },
        onPointerCancel: release,
        onLostPointerCapture: release,
    };
}
