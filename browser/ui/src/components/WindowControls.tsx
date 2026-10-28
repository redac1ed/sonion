import React from "react";
import {WindowCommand} from "../types";

interface WindowControlsProps {
    maximized: boolean;
    onWindowCommand: (cmd: WindowCommand) => void;
}

export const WindowControls: React.FC<WindowControlsProps> = ({maximized, onWindowCommand}) => {
    return (
        <div className="window-controls">
            <button className="win-btn" onClick={() => onWindowCommand("win_minimize")} title="Minimize">
                <svg viewBox="0 0 10 10" shapeRendering="crispEdges">
                    <path d="M0 5.5h10" />
                </svg>
            </button>
            <button className="win-btn" onClick={() => onWindowCommand("win_toggle_maximize")} title={maximized ? "Restore" : "Maximize"}>
                {maximized ? (
                    <svg viewBox="0 0 10 10" shapeRendering="crispEdges">
                        <rect x="0.5" y="2.5" width="7" height="7" />
                        <path d="M2.5 2.5v-2h7v7h-2" />
                    </svg>
                ) : (
                    <svg viewBox="0 0 10 10" shapeRendering="crispEdges">
                        <rect x="0.5" y="0.5" width="9" height="9" />
                    </svg>
                )}
            </button>
            <button className="win-btn win-close" onClick={() => onWindowCommand("exit")} title="Close">
                <svg viewBox="0 0 10 10">
                    <path d="M0.5 0.5l9 9M9.5 0.5l-9 9" />
                </svg>
            </button>
        </div>
    )
}