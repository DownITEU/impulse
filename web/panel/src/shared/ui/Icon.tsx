import type { SVGProps } from 'react';

const ICON_PATHS = {
    alert: (
        <>
            <path d="M12 3 2.8 20h18.4z" />
            <path d="M12 9v4" />
            <path d="M12 17h.01" />
        </>
    ),
    'arrow-right': (
        <>
            <path d="M5 12h14" />
            <path d="m13 6 6 6-6 6" />
        </>
    ),
    bolt: <path d="m13 2-8 12h6l-1 8 8-12h-6z" />,
    'chevron-down': <path d="m6 9 6 6 6-6" />,
    dot: <circle cx="12" cy="12" r="4" fill="currentColor" stroke="none" />,
    'gamepad-variant-outline': (
        <path
            fill="currentColor"
            stroke="none"
            d="M6,9H8V11H10V13H8V15H6V13H4V11H6V9M18.5,9A1.5,1.5 0 0,1 20,10.5A1.5,1.5 0 0,1 18.5,12A1.5,1.5 0 0,1 17,10.5A1.5,1.5 0 0,1 18.5,9M15.5,12A1.5,1.5 0 0,1 17,13.5A1.5,1.5 0 0,1 15.5,15A1.5,1.5 0 0,1 14,13.5A1.5,1.5 0 0,1 15.5,12M17,5A7,7 0 0,1 24,12A7,7 0 0,1 17,19C15.04,19 13.27,18.2 12,16.9C10.73,18.2 8.96,19 7,19A7,7 0 0,1 0,12A7,7 0 0,1 7,5H17M7,7A5,5 0 0,0 2,12A5,5 0 0,0 7,17C8.64,17 10.09,16.21 11,15H13C13.91,16.21 15.36,17 17,17A5,5 0 0,0 22,12A5,5 0 0,0 17,7H7Z"
        />
    ),
    list: (
        <>
            <path d="M4 6h16" />
            <path d="M4 12h16" />
            <path d="M4 18h10" />
            <circle cx="3" cy="6" r=".6" fill="currentColor" />
            <circle cx="3" cy="12" r=".6" fill="currentColor" />
            <circle cx="3" cy="18" r=".6" fill="currentColor" />
        </>
    ),
    loader: (
        <>
            <path d="M12 3a9 9 0 1 0 9 9" />
            <path d="M21 12a9 9 0 0 0-9-9" />
        </>
    ),
    menu: (
        <>
            <path d="M4 7h12" />
            <path d="M4 12h16" />
            <path d="M4 17h8" />
        </>
    ),
    minus: <path d="M5 12h14" />,
    pin: (
        <path
            fill="currentColor"
            stroke="none"
            d="M16,12V4H17V2H7V4H8V12L6,14V16H11.2V22H12.8V16H18V14L16,12Z"
        />
    ),
    'pin-off': (
        <path
            fill="currentColor"
            stroke="none"
            d="M2,5.27L3.28,4L20,20.72L18.73,22L12.8,16.07V22H11.2V16H6V14L8,12V11.27L2,5.27M16,12L18,14V16H17.82L8,6.18V4H7V2H17V4H16V12Z"
        />
    ),
    plug: (
        <>
            <path d="M8 2v6" />
            <path d="M16 2v6" />
            <path d="M7 8h10v4a5 5 0 0 1-10 0z" />
            <path d="M12 17v5" />
        </>
    ),
    play: <path d="m8 5 11 7-11 7z" />,
    plus: (
        <>
            <path d="M12 5v14" />
            <path d="M5 12h14" />
        </>
    ),
    search: (
        <>
            <circle cx="11" cy="11" r="7" />
            <path d="m20 20-4-4" />
        </>
    ),
    sparkles: (
        <>
            <path d="m12 3 1.6 5.4L19 10l-5.4 1.6L12 17l-1.6-5.4L5 10l5.4-1.6z" />
            <path d="m5 16 .8 2.2L8 19l-2.2.8L5 22l-.8-2.2L2 19l2.2-.8z" />
        </>
    ),
    star: <path d="m12 3 2.7 5.5 6.1.9-4.4 4.3 1 6.1-5.4-2.9-5.4 2.9 1-6.1-4.4-4.3 6.1-.9z" />,
    'star-filled': (
        <path
            fill="currentColor"
            stroke="none"
            d="m12 3 2.7 5.5 6.1.9-4.4 4.3 1 6.1-5.4-2.9-5.4 2.9 1-6.1-4.4-4.3 6.1-.9z"
        />
    ),
    stop: <rect x="6" y="6" width="12" height="12" rx="1.5" fill="currentColor" stroke="none" />,
    x: (
        <>
            <path d="M6 6l12 12" />
            <path d="M18 6 6 18" />
        </>
    ),
};

export type IconName = keyof typeof ICON_PATHS;

type IconProps = Omit<SVGProps<SVGSVGElement>, 'stroke'> & {
    name: IconName;
    stroke?: number | string;
};

export function Icon({ name, className, stroke = 1.8, ...props }: IconProps) {
    return (
        <svg
            aria-hidden="true"
            className={className}
            fill="none"
            stroke="currentColor"
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={stroke}
            viewBox="0 0 24 24"
            {...props}
        >
            {ICON_PATHS[name]}
        </svg>
    );
}
