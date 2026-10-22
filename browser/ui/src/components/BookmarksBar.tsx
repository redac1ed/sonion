import React from "react";
import BookmarkBorderIcon from "@mui/icons-material/BookmarkBorder";
import {BookmarkItem} from "../types";

interface BookmarksBarProps {
    bookmarks: BookmarkItem[];
    onSelectBookmark: (url: string) => void;
}

export const BookmarksBar: React.FC<BookmarksBarProps> = ({
    bookmarks,
    onSelectBookmark
}) => {
    return (
        <div className="bookmarks-bar">
            {bookmarks.map((bm) => (
                <button
                    key={bm.id}
                    className="bookmark-pill"
                    onClick={() => onSelectBookmark(bm.url)}
                    title={bm.url}
                >
                    <BookmarkBorderIcon sx={{fontSize: 14, color: "#67e8f9"}} />
                    <span>{bm.title}</span>
                </button>
            ))}
        </div>
    );
};