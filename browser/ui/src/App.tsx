import React, {useEffect, useState} from "react";
import {BookmarkItem, BrowserSettings, DownloadItem, HistoryEntry, NativeHostEvent, PanelMode, Profile, SaveResult, TabItem} from "./types";
import {extractDisplayTitle, normalizeSonionUrl, sendIpc} from "./ipc";
import {TabStrip} from "./components/TabStrip";
import {Toolbar} from "./components/Toolbar";
import {BookmarksBar} from "./components/BookmarksBar";
import {SidePanel} from "./components/SidePanel";
import {FindBar} from "./components/FindBar";

const BOOT_SETTINGS: BrowserSettings = window.__SONION_SETTINGS__ ?? {
    registry: "sonion://87.192.41.2:6767",
    homepage: "sonion://cool.rizz",
    pin_key: "",
    insecure_dev: true
};
const NEW_TAB_URL = "sonion-nav://home";
const BOOT_PROFILE: Partial<Profile> | null = window.__SONION_PROFILE__ ?? null;
const INITIAL_BOOKMARKS: BookmarkItem[] = [
    {id: "bm-1", title: "cool.rizz", url: "sonion://cool.rizz"},
    {id: "bm-2", title: "localhost:6767", url: "sonion://localhost:6767"},
    {id: "bm-3", title: "New Tab", url: "sonion-nav://home"}
];
let nextUid = 2;

function createTabObject(id: string, url: string): TabItem {
    return {
        id, 
        title: extractDisplayTitle(url),
        url, 
        history: [url],
        historyIndex: 0,
        loading: false,
        secure: url.startsWith("sonion://")
    };
}

export const App: React.FC = () => {
    const [tabs, setTabs] = useState<TabItem[]>(() => [
        createTabObject("tab-1", NEW_TAB_URL)
    ]);
    const [activeTabId, setActiveTabId] = useState<string>("tab-1");
    const [addressInput, setAddressInput] = useState<string>("");
    const [bookmarks, setBookmarks] = useState<BookmarkItem[]>(BOOT_PROFILE?.bookmarks ?? INITIAL_BOOKMARKS);
    const [globalHistory, setGlobalHistory] = useState<HistoryEntry[]>(BOOT_PROFILE?.history ?? []);
    const [zoom, setZoom] = useState<number>(100);
    const [settings, setSettings] = useState<BrowserSettings>(BOOT_SETTINGS);
    const [saveResult, setSaveResult] = useState<SaveResult | null>(null); 
    const [downloads, setDownloads] = useState<DownloadItem[]>([]);
    const [findOpen, setFindOpen] = useState<boolean>(false);
    const [findQuery, setFindQuery] = useState<string>("");
    const [findCount, setFindCount] = useState({current: 0, total: 0});
    const [findFocus, setFindFocus] = useState<number>(0);
    const [panelMode, setPanelMode] = useState<PanelMode>("none");
    const [maximized, setMaximized] = useState<boolean>(false);
    const activeTab = tabs.find((t) => t.id === activeTabId) || tabs[0];
    const isBookmarked = bookmarks.some((b) => b.url === activeTab.url);
    const downloading = downloads.some((d) => d.state === "in_progress");
    const showBookmarkRow = bookmarks.length > 0 || findOpen;
    const chromeHeight = showBookmarkRow ? 114 : 84;

    const openFind = () => {
        setPanelMode("none");
        setFindOpen(true);
        setFindFocus((n) => n + 1);
    };

    const closeFind = () => {
        if (!findOpen) return;
        setFindOpen(false);
        setFindQuery("");
        setFindCount({current: 0, total: 0});
        sendIpc({cmd: "find_clear", tabId: activeTab.id});
    };

    const runFind = (query: string, dir: number) => {
        if (!query) {
            setFindCount({current: 0, total: 0});
            sendIpc({cmd: "find_clear", tabId: activeTab.id});
            return;
        }
        sendIpc({cmd: "find", tabId: activeTab.id, query, dir});
    };

    useEffect(() => {
        sendIpc({cmd: "set_chrome_height", height: chromeHeight, overlay: panelMode !== "none"});
    }, [panelMode, chromeHeight]);

    useEffect(() => {
        setSaveResult(null);
    }, [panelMode]);

    useEffect(() => {
        const timer = setTimeout(() => {
            sendIpc({cmd: "save_profile", profile: {bookmarks, history: globalHistory.slice(0, 1000)}});
        }, 400);
        return () => clearTimeout(timer);
    }, [bookmarks, globalHistory]);

    useEffect(() => {
        window._onNativeEvent = (ev: NativeHostEvent) => {
            if (ev.event === "window_state") {
                setMaximized(!!ev.maximized);
                return;
            }
            if (ev.event === "settings_saved") {
                if (ev.ok && ev.settings) setSettings(ev.settings);
                setSaveResult({ok: !!ev.ok, error: ev.error});
                if (ev.ok) setTimeout(() => setSaveResult(null), 1800);
                return;
            }
            if (ev.event === "download" && ev.download) {
                const item = ev.download;
                setDownloads((prev) =>
                    prev.some((d) => d.id === item.id)
                        ? prev.map((d) => (d.id === item.id ? item : d))
                        : [item, ...prev]
                );
                if (item.state === "in_progress") setPanelMode("downloads");
                return;
            }
            if (ev.event === "find_result") {
                setFindCount({current: ev.current ?? 0, total: ev.total ?? 0});
                return;
            }
            if (ev.event === "shortcut") {
                if (ev.key === "f") openFind();
                return;
            }
            if (ev.event === "open" && ev.url) {
                navigateTab(ev.tabId, ev.url, true);
                return;
            }
            setTabs((prev) => 
                prev.map((t) => {
                    if (t.id !== ev.tabId) return t;
                    return {
                        ...t, 
                        loading: ev.event === "page_started" ? true : ev.event === "page_loaded" ? false : t.loading,
                        title: ev.title ?? t.title,
                        url: ev.url ?? t.url
                    };
                })
            );
        };
    }, []);

    useEffect(() => {
        const onKeyDown = (e: KeyboardEvent) => {
            if (e.ctrlKey && e.key.toLowerCase() === "t") {
                e.preventDefault();
                handleNewTab();
            } else if (e.ctrlKey && e.key.toLowerCase() === "w") {
                e.preventDefault();
                handleCloseTab(activeTab.id);
            } else if (e.ctrlKey && e.key.toLowerCase() === "r") {
                e.preventDefault();
                handleReload();
            } else if (e.ctrlKey && e.key.toLowerCase() === "f") {
                e.preventDefault();
                openFind();
            }
        };
        window.addEventListener("keydown", onKeyDown);
        return () => window.removeEventListener("keydown", onKeyDown);
    });

    const navigateTab = (
        tabId: string,
        rawTarget: string,
        pushHistory: boolean,
    ) => {
        const target = normalizeSonionUrl(rawTarget);
        if (!target) return;
        if (target.startsWith("http://") || target.startsWith("https://")) {
            sendIpc({cmd: "external", url: target});
            return;
        }
        setTabs((prev) => 
            prev.map((tab) => {
                if (tab.id !== tabId) return tab;
                const nextHistory = pushHistory ? [...tab.history.slice(0, tab.historyIndex + 1), target] : tab.history;
                const nextIndex = pushHistory ? nextHistory.length - 1 : tab.historyIndex;
                return {
                    ...tab, 
                    url: target, 
                    title: extractDisplayTitle(target),
                    history: nextHistory,
                    historyIndex: nextIndex,
                    loading: true,
                    secure: target.startsWith("sonion://")
                };
            })
        );
        if (tabId === activeTabId) {
            setAddressInput(target.startsWith("sonion-nav://home") ? "" : target);
        }
        if (pushHistory && !target.startsWith("sonion-nav://")) {
            setGlobalHistory((prev) => [
                {
                    id: `h-${Date.now()}`,
                    title: extractDisplayTitle(target),
                    url: target,
                    timestamp: Date.now()
                },
                ...prev
            ]);
        }
        sendIpc({cmd: "nav", tabId, url: target});
    };

    const handleSelectTab = (tabId: string) => {
        const targetTab = tabs.find((t) => t.id === tabId);
        if (!targetTab) return;
        setActiveTabId(tabId);
        setAddressInput(targetTab.url.startsWith("sonion-nav://home") ? "" : targetTab.url);
        sendIpc({cmd: "switch_tab", tabId});
    };

    const handleNewTab = () => {
        const newId = `tab-${nextUid++}`;
        const initialUrl = "sonion-nav://home";
        const newTab = createTabObject(newId, initialUrl);
        setTabs((prev) => [...prev, newTab]);
        setActiveTabId(newId);
        setAddressInput("");
        sendIpc({cmd: "create_tab", tabId: newId, url: initialUrl});
    };

    const handleCloseTab = (tabId: string, e?: React.MouseEvent) => {
        e?.stopPropagation();
        if (tabs.length === 1) {
            navigateTab(tabs[0].id, "sonion-nav://home", true);
            return;
        }
        const idx = tabs.findIndex((t) => t.id === tabId);
        const remaining = tabs.filter((t) => t.id !== tabId);
        setTabs(remaining);
        sendIpc({cmd: "close_tab", tabId});
        if (tabId === activeTabId) {
            const nextActive = remaining[Math.max(0, idx - 1)];
            setActiveTabId(nextActive.id);
            setAddressInput(nextActive.url.startsWith("sonion-nav://home") ? "" : nextActive.url);
            sendIpc({cmd: "switch_tab", tabId: nextActive.id});
        }
    };

    const handleBack = () => {
        if (activeTab.historyIndex <= 0) return;
        const nextIdx = activeTab.historyIndex - 1;
        const targetUrl = activeTab.history[nextIdx];
        setTabs((prev) => 
            prev.map((t) => 
                t.id === activeTab.id ? {
                    ...t, 
                    historyIndex: nextIdx,
                    url: targetUrl,
                    title: extractDisplayTitle(targetUrl),
                    loading: true
                } : t
            )
        );
        setAddressInput(targetUrl.startsWith("sonion-nav://home") ? "" : targetUrl);
        sendIpc({cmd: "nav", tabId: activeTab.id, url: targetUrl});
    };

    const handleForward = () => {
        if (activeTab.historyIndex >= activeTab.history.length - 1) return;
        const nextIdx = activeTab.historyIndex + 1;
        const targetUrl = activeTab.history[nextIdx];
        setTabs((prev) => 
            prev.map((t) => 
                t.id === activeTab.id ? {
                    ...t,
                    historyIndex: nextIdx,
                    url: targetUrl,
                    title: extractDisplayTitle(targetUrl)
                } : t 
            )
        );
        setAddressInput(targetUrl.startsWith("sonion-nav://home") ? "": targetUrl);
        sendIpc({cmd: "nav", tabId: activeTab.id, url: targetUrl});
    };

    const handleReload = () => {
        setTabs((prev) => prev.map((t) => (t.id === activeTab.id ? {...t, loading: true} : t)));
        sendIpc({cmd: "reload", tabId: activeTab.id})
    };

    const handleHome = () => {
        navigateTab(activeTab.id, settings.homepage, true);
    };

    const handleToggleBookmark = () => {
        if (isBookmarked) {
            setBookmarks((prev) => prev.filter((b) => b.url !== activeTab.url));
        } else {
            setBookmarks((prev) => [
                ...prev,
                {
                    id: `bm-${Date.now()}`,
                    title: activeTab.title || extractDisplayTitle(activeTab.url),
                    url: activeTab.url
                }
            ]);
        }
    };

    return (
        <div className="chrome-shell">
            <TabStrip   
                tabs={tabs}
                activeTabId={activeTab.id}
                onSelectTab={handleSelectTab}
                onCloseTab={handleCloseTab}
                onNewTab={handleNewTab}
                maximized={maximized}
                onWindowCommand={(cmd) => sendIpc({cmd})}
            />
            <Toolbar
                activeTab={activeTab}
                addressInput={addressInput}
                isBookmarked={isBookmarked}
                panelMode={panelMode}
                downloadCount={downloads.length}
                downloading={downloading}
                onAddressChange={setAddressInput}
                onNavigateSubmit={() => navigateTab(activeTab.id, addressInput, true)}
                onBack={handleBack}
                onForward={handleForward}
                onReload={handleReload}
                onHome={handleHome}
                onToggleBookmark={handleToggleBookmark}
                onTogglePanel={setPanelMode}
                onOpenDevtools={() => sendIpc({cmd: "devtools", tabId: activeTab.id})}
            />
            {showBookmarkRow && (
                <BookmarksBar
                    bookmarks={bookmarks}
                    onSelectBookmark={(url) => navigateTab(activeTab.id, url, true)}
                />
            )}
            {findOpen && (
                <FindBar
                    query={findQuery}
                    current={findCount.current}
                    total={findCount.total}
                    focusToken={findFocus}
                    onQueryChange={(query) => {
                        setFindQuery(query);
                        runFind(query, 0);
                    }}
                    onNext={() => runFind(findQuery, 1)}
                    onPrev={() => runFind(findQuery, -1)}
                    onClose={closeFind}
                />
            )}
            <SidePanel
                mode={panelMode}
                setMode={setPanelMode}
                historyEntries={globalHistory}
                bookmarks={bookmarks}
                downloads={downloads}
                settings={settings}
                saveResult={saveResult}
                zoom={zoom}
                onZoomChange={(nextZoom) => {
                    setZoom(nextZoom);
                    sendIpc({cmd: "zoom", tabId: activeTab.id, zoom: nextZoom});
                }}
                onFullscreen={() => sendIpc({cmd: "fullscreen"})}
                onNewTab={handleNewTab}
                onNewWindow={() => sendIpc({cmd: "new_window"})}
                onPrint={() => sendIpc({cmd: "print", tabId: activeTab.id})}
                onSavePage={() => sendIpc({cmd: "save_page", tabId: activeTab.id})}
                onFind={openFind}
                onExit={() => sendIpc({cmd: "exit"})}
                onSaveSettings={(next) => {
                    setSaveResult(null);
                    sendIpc({cmd: "save_settings", settings: next});
                }}
                onNavigate={(url) => navigateTab(activeTab.id, url, true)}
                onClearHistory={() => setGlobalHistory([])}
                onOpenDownload={(path) => sendIpc({cmd: "open_download", path})}
                onShowDownload={(path) => sendIpc({cmd: "show_download", path})}
                onClearDownloads={() => setDownloads((prev) => prev.filter((d) => d.state === "in_progress"))}
                onClose={() => setPanelMode("none")}
            />
        </div>
    );
};