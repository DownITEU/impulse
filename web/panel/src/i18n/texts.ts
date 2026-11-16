import { FluentBundle, FluentResource, type FluentVariable } from '@fluent/bundle';
import { createContext, useContext } from 'react';

/** `locales/<tag>.ftl`, by tag. */
const SOURCES = new Map(
    Object.entries(
        import.meta.glob<string>('../../locales/*.ftl', {
            query: '?raw',
            import: 'default',
            eager: true,
        }),
    ).map(([path, source]) => [path.slice(path.lastIndexOf('/') + 1, -'.ftl'.length), source]),
);

function bundle(tag: string): FluentBundle {
    const made = new FluentBundle(tag, { useIsolating: false });
    made.addResource(new FluentResource(SOURCES.get(tag) ?? ''));
    return made;
}

const ENGLISH = bundle('en');

export type T = (id: string, args?: Record<string, FluentVariable>) => string;

/** The app's language: its tag, for numbers, and its captions. */
export type Words = { tag: string; t: T };

/** Captions in `tag`, English for a message it lacks and for a language the panel does not ship. */
export function texts(tag: string): Words {
    const bundles = SOURCES.has(tag) && tag !== 'en' ? [bundle(tag), ENGLISH] : [ENGLISH];
    const t: T = (id, args) => {
        for (const source of bundles) {
            const message = source.getMessage(id);
            if (message?.value) return source.formatPattern(message.value, args);
        }
        return id;
    };
    return { tag, t };
}

export const Texts = createContext<Words>(texts('en'));

/** Captions in the app's language; a component that uses them follows a change. */
export const useT = (): T => useContext(Texts).t;

/** The app's language tag: numbers follow it as captions do. */
export const useLanguage = (): string => useContext(Texts).tag;
