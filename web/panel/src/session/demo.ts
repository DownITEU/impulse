import type { View } from '@/api';

import { opened, receive } from './store';

/** `pnpm dev`, then `/?demo`: views as the app would send them, without the app. */
export function play(): void {
    const views: View[] = [
        {
            type: 'hello',
            version: 1,
            language: new URLSearchParams(location.search).get('demo') || 'en',
            accent: '#b4f000',
        },
        {
            type: 'library',
            games: [
                {
                    game: 0,
                    name: 'ENSHROUDED',
                    favorite: true,
                    open: true,
                    live: true,
                    startable: false,
                    art: false,
                },
                {
                    game: 1,
                    name: 'ICARUS',
                    favorite: false,
                    open: false,
                    live: false,
                    startable: true,
                    art: false,
                },
                {
                    game: 2,
                    name: 'RIMWORLD',
                    favorite: false,
                    open: false,
                    live: false,
                    startable: false,
                    art: false,
                },
            ],
        },
        {
            type: 'trainer',
            sheet: 1,
            game: 0,
            trainer: {
                state: 'ready',
                sections: [
                    { name: 'PINNED', pinned: true, cheats: [0] },
                    { name: 'PLAYER', pinned: false, cheats: [1, 2, 3] },
                    { name: 'WORLD', pinned: false, cheats: [4, 5] },
                ],
                cheats: [
                    {
                        name: 'Unlimited Health',
                        description: null,
                        instructions: null,
                        pinned: true,
                        control: { kind: 'toggle' },
                    },
                    {
                        name: 'Unlimited Stamina',
                        description: 'Never tire.',
                        instructions: null,
                        pinned: false,
                        control: { kind: 'toggle' },
                    },
                    {
                        name: 'Movement Speed',
                        description: null,
                        instructions: null,
                        pinned: false,
                        control: { kind: 'slider', min: 0.5, max: 3, step: 0.5 },
                    },
                    {
                        name: 'Add Gold',
                        description: null,
                        instructions: 'Open the inventory first.',
                        pinned: false,
                        control: { kind: 'number', min: 0, max: 99999, step: 100, apply: true },
                    },
                    {
                        name: 'Time of Day',
                        description: null,
                        instructions: null,
                        pinned: false,
                        control: { kind: 'selection', options: ['Dawn', 'Noon', 'Dusk', 'Night'] },
                    },
                    {
                        name: 'Save Position',
                        description: null,
                        instructions: null,
                        pinned: false,
                        control: { kind: 'button' },
                    },
                ],
                collections: [{ name: 'Explore', icon: 'rocket' }],
            },
        },
        { type: 'values', sheet: 1, values: [1, 0, 1.5, 0, 2, 0] },
        {
            type: 'run',
            state: 'live',
            game: 0,
            since: Math.floor(Date.now() / 1000) - 754,
            why: null,
        },
    ];
    opened();
    views.forEach(receive);
}
