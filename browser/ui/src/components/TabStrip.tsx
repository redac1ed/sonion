import React from "react";
import AddIcon from "@mui/icons-material/Add";
import CloseIcon from "@mui/icons-material/Close";
import PublicIcon from "@mui/icons-material/Public";
import AutorenewIcon from "@mui/icons-material/Autorenew";
import { TabItem } from "../types";

interface TabStripProps {
    tabs: TabItem[];
    activeTabId: string;
    onSelectTab: (tabId: string) => void;
    onCloseTab: (tabId: string, e: React.MouseEvent) => void;
    onNewTab: () => void;
}

export const TabStrip: React.FC<TabStripProps> = ({
    tabs, 
    activeTabId,
    onSelectTab,
    onCloseTab,
    onNewTab
}) => {
    return (
        <div className="tab-strip">
            {tabs.map((tab) => {
                const isActive = tab.id === activeTabId;
                return (
                    <div
                        key={tab.id}
                        className={`tab-item ${isActive ? "active" : ""}`}
                        onClick={() => onSelectTab(tab.id)}
                        title={tab.url}
                    >
                        {tab.loading ? (
                        <AutorenewIcon sx={{ fontSize: 15, color: "#67e8f9" }} />
                        ) : (
                            <PublicIcon
                                sx={{
                                fontSize: 15,
                                color: isActive ? "#67e8f9" : "#949ba4",
                            }}/>
                        )}
                        <span className="tab-title">{tab.title || "New Tab"}</span>
                        <button
                            className="tab-close-btn"
                            onClick={(e) => onCloseTab(tab.id, e)}
                            title="Close tab"
                        >
                        <CloseIcon sx={{ fontSize: 14 }} />
                        </button>
                    </div>
                );
            })}
            <button className="new-tab-btn" onClick={onNewTab} title="New tab (Ctrl+T)">
                <AddIcon sx={{ fontSize: 18 }} />
            </button>
        </div>
    );
};