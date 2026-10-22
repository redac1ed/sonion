import React, {useEffect, useState} from "react";
import {BookmarkItem, HistoryEntry, NativeHostEvent, PanelMode, TabItem} from "./types";
import {extractDisplayTitle, normalizeSonionUrl, sendIpc} from "./ipc";
import {TabStrip} from "./components/TabStrip";
import {Toolbar} from "./components/Toolbar";
import {BookmarksBar} from "./components/BookmarksBar";
import {SidePanel} from "./components/SidePanel";

const DEFAULT_HOME = "sonion://cool.rizz";
const INITIAL_BOOKMARKS: BookmarkItem[] = [
    {id: "bm-1", title: "cool.rizz", url: "sonion://cool.rizz"},
    {id: "bm-2", title: "localhost:6767", url: "sonion://localhost:6767"},
    {id: "bm-3", title: "New Tab", url: "sonion-nav://home"}
];

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
        createTabObject("tab-1", DEFAULT_HOME)
    ]);
    const [activeTabId, setActiveTabId] = useState<string>("tab-1");
    const [addressInput, setAddressInput] = useState<string>(DEFAULT_HOME);
    const [bookmarks, setBookmarks] = useState<BookmarkItem[]>(INITIAL_BOOKMARKS);
    const [globalHistory, setGlobalHistory] = useState<HistoryEntry[]>([
        {
            id: "h-init",
            title: "cool.rizz",
            url: DEFAULT_HOME,
            timestamp: Date.now()
        }
    ]);
    const [panelMode, setPanelMode] = useState<PanelMode>("none");
    const activeTab = tabs.find((t) => t.id === activeTabId) || tabs[0];
    const isBookmarked = bookmarks.some((b) => b.url === activeTab.url);

    useEffect(() => {
        const baseHeight = 114;
        const drawerExtra = panelMode === "none" ? 0 : 140;
        sendIpc({cmd: "set_chrome_height", height: baseHeight + drawerExtra});
    }, [panelMode]);
    useEffect(() => {
        window._onNativeEvent = (ev: NativeHostEvent) => {
            setTabs((prev) => 
                prev.map((t) => {
                    if (t.id !== ev.tabId) return t;
                    return {
                        ...t, 
                        loading: ev.event === "page_loaded" ? false : t.loading,
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
                    loading: false,
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
        const newId = `tab-${Date.now()}`;
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
        sendIpc({cmd: "reload", tabId: activeTab.id})
    };

    const handleHome = () => {
        navigateTab(activeTab.id, DEFAULT_HOME, true);
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
            />
            <Toolbar
                activeTab={activeTab}
                addressInput={addressInput}
                isBookmarked={isBookmarked}
                panelMode={panelMode}
                onAddressChange={setAddressInput}
                onNavigateSubmit={() => navigateTab(activeTab.id, addressInput, true)}
                onBack={handleBack}
                onForward={handleReload}
                onReload={handleReload}
                onHome={handleHome}
                onToggleBookmark={handleToggleBookmark}
                onTogglePanel={setPanelMode}
                onOpenDevtools={() => sendIpc({cmd: "devtools", tabId: activeTab.id})}
            />
            <BookmarksBar
                bookmarks={bookmarks}
                onSelectBookmark={(url) => navigateTab(activeTab.id, url, true)}
            />
            <SidePanel
                mode={panelMode}
                historyEntries={globalHistory}
                onNavigate={(url) => navigateTab(activeTab.id, url, true)}
                onClearHistory={() => setGlobalHistory([])}
                onClose={() => setPanelMode("none")}
            />
        </div>
    );
};