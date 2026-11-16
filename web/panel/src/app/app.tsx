import { useEffect, useMemo, useState } from 'react';

import { ConnectionDrawer } from '@/app/ui/ConnectionDrawer';
import { FloatingDock } from '@/app/ui/FloatingDock';
import { Notes } from '@/app/ui/Notes';
import { blocked, SessionPlaceholder } from '@/app/ui/SessionPlaceholder';
import { TopBar } from '@/app/ui/TopBar';
import { Texts, texts } from '@/i18n/texts';
import { isRunning } from '@/library/model/games';
import { GameHeader } from '@/library/ui/GameHeader';
import { LibraryDrawer } from '@/library/ui/LibraryDrawer';
import { isLive, valuesOf } from '@/session/state';
import { useSession } from '@/session/store';
import { Drawer } from '@/shared/ui/Drawer';

import { TrainerSheet } from '@/trainer/ui/TrainerSheet';

import { useDockAutoHide } from './use-dock-auto-hide';

export const App = () => {
    const state = useSession();
    const language = state.hello?.language ?? 'en';
    const accent = state.hello?.accent;
    const words = useMemo(() => texts(language), [language]);
    const [drawer, setDrawer] = useState<'connection' | 'library' | null>(null);
    const dock = useDockAutoHide();
    const { t } = words;

    useEffect(() => {
        document.documentElement.lang = language;
    }, [language]);
    // On the root: the body and the dialog's top layer read it too.
    useEffect(() => {
        if (accent) document.documentElement.style.setProperty('--deck-accent', accent);
    }, [accent]);

    const game = state.games.find((one) => one.game === state.trainer?.game) ?? null;
    const playing = state.games.find((one) => isRunning(state.run, one)) ?? null;
    const why = blocked(state, game);
    const online = state.link === 'open';
    const close = () => setDrawer(null);
    const openLibrary = () => setDrawer('library');
    const openConnection = () => setDrawer('connection');

    return (
        <Texts.Provider value={words}>
            <main className="min-h-svh bg-[#050608] text-(--deck-fg)">
                <section className="relative h-svh w-full overflow-hidden bg-(--deck-bg)">
                    <div className="pointer-events-none absolute -inset-12 z-0 bg-[radial-gradient(circle_at_30%_15%,color-mix(in_oklab,var(--deck-accent)_22%,transparent),transparent_45%),radial-gradient(circle_at_80%_85%,color-mix(in_oklab,var(--deck-accent)_16%,transparent),transparent_45%),radial-gradient(circle_at_20%_80%,color-mix(in_oklab,var(--deck-accent)_8%,transparent),transparent_50%)]" />
                    <div className="pointer-events-none absolute inset-0 z-0 bg-[radial-gradient(ellipse_100%_60%_at_50%_0%,rgba(255,255,255,0.025),transparent)]" />
                    <div className="relative z-10 flex h-full flex-col">
                        <TopBar
                            link={state.link}
                            title={why ? null : (game?.name ?? null)}
                            onOpenConnection={openConnection}
                        />
                        <div
                            className="remote-scrollbar-hidden min-h-0 flex-1 overflow-y-auto overscroll-contain px-3.5 pb-27.5"
                            onScroll={dock.onScroll}
                        >
                            {why || !game || !state.trainer ? (
                                <SessionPlaceholder
                                    why={why ?? 'pick'}
                                    onOpenLibrary={openLibrary}
                                />
                            ) : (
                                <>
                                    <GameHeader game={game} run={state.run} online={online} />
                                    <TrainerSheet
                                        key={game.game}
                                        trainer={state.trainer}
                                        values={valuesOf(state)}
                                        online={online}
                                        live={isLive(state)}
                                    />
                                </>
                            )}
                        </div>
                    </div>

                    <Notes notes={state.notes} />
                    <FloatingDock
                        playing={playing?.name ?? null}
                        live={state.run?.state === 'live'}
                        hidden={dock.hidden}
                        onOpenConnection={openConnection}
                        onOpenLibrary={openLibrary}
                    />

                    <Drawer
                        open={drawer === 'connection'}
                        side="left"
                        label={t('connection-title')}
                        onClose={close}
                    >
                        <ConnectionDrawer link={state.link} onClose={close} />
                    </Drawer>
                    <Drawer
                        open={drawer === 'library'}
                        side="right"
                        label={t('library-title')}
                        onClose={close}
                    >
                        <LibraryDrawer
                            games={state.games}
                            run={state.run}
                            online={online}
                            onClose={close}
                        />
                    </Drawer>
                </section>
            </main>
        </Texts.Provider>
    );
};
