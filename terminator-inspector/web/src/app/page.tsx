"use client";

import { invoke } from '@tauri-apps/api/tauri';
import { RefreshCcw } from 'lucide-react';
import { useEffect, useState } from 'react';
import ErrorBanner from '../components/ErrorBanner';
import TreeView, { SerializableNode } from '../components/TreeView';

export default function HomePage() {
    const [tree, setTree] = useState<SerializableNode[]>([]);
    const [errorMsg, setErrorMsg] = useState<string | null>(null);

    useEffect(() => {
        load();
    }, []);

    const load = () => {
        invoke('get_ui_tree')
            .then((res) => {
                setTree(res as any[]);
                setErrorMsg(null);
            })
            .catch((err) => {
                console.error(err);
                setErrorMsg(String(err));
            });
    };

    return (
        <main className="p-4">
            <header className="sticky top-0 bg-white/80 backdrop-blur mb-4 flex items-center justify-between p-2 border-b">
                <h1 className="text-xl font-semibold">Terminator Inspector</h1>
                <button
                    onClick={load}
                    className="flex items-center gap-1 text-sm text-blue-600 hover:underline"
                >
                    <RefreshCcw size={16} /> Refresh
                </button>
            </header>
            {errorMsg && <ErrorBanner message={errorMsg} onClose={() => setErrorMsg(null)} />}
            {tree.length === 0 ? (
                <p className="text-sm text-gray-600">Loading UI tree...</p>
            ) : (
                <TreeView nodes={tree} />
            )}
        </main>
    );
}