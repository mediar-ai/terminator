"use client";

interface Props {
    message: string;
    onClose(): void;
}

export default function ErrorBanner({ message, onClose }: Props) {
    return (
        <div className="bg-red-100 border border-red-400 text-red-800 px-4 py-2 rounded relative mb-4">
            <span>{message}</span>
            <button
                onClick={onClose}
                className="absolute right-2 top-1 text-red-900 font-bold"
                aria-label="Close error"
            >
                ×
            </button>
        </div>
    );
}