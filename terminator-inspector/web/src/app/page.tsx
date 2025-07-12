"use client";

import { Button } from '@/components/ui/button';
import { Skeleton } from '@/components/ui/skeleton';
import { invoke } from '@tauri-apps/api/tauri';
import { RefreshCcw } from 'lucide-react';
import { useEffect, useState } from 'react';
import ErrorBanner from '../components/ErrorBanner';
import TreeView, { SerializableNode } from '../components/TreeView';

export default function HomePage() {
    const [tree, setTree] = useState<SerializableNode[]>([]);
    const [errorMsg, setErrorMsg] = useState<string | null>(null);
    const [loading, setLoading] = useState(false);

    const collectExpandable = (nodes: SerializableNode[], prefix = ''): string[] => {
        const arr: string[] = [];
        nodes.forEach((n, idx) => {
            const path = prefix ? `${prefix}.${idx}` : `${idx}`;
            if (n.children && n.children.length > 0) {
                arr.push(path);
                arr.push(...collectExpandable(n.children, path));
            }
        });
        return arr;
    };

    useEffect(() => {
        load();
    }, []);

    const load = () => {
        setLoading(true);
        invoke('get_ui_tree')
            .then((res) => {
                setTree(res as any[]);
                setErrorMsg(null);
                setLoading(false);
            })
            .catch((err) => {
                console.error(err);
                setErrorMsg(String(err));
                setLoading(false);
            });
    };

    const handleExpandAll = () => {
        const all = collectExpandable(tree);
        setOpen(all);
    };

    const handleCollapseAll = () => setOpen([]);

    const [open, setOpen] = useState<string[]>([]);

    return (
        <main className="p-4">
            <header className="sticky top-0 bg-white/80 backdrop-blur mb-4 flex flex-wrap gap-2 items-center justify-between p-2 border-b">
                <h1 className="text-xl font-semibold">Terminator Inspector</h1>
                <div className="flex gap-2">
                    <Button variant="ghost" size="sm" onClick={load} disabled={loading}>
                        <RefreshCcw size={16} className="mr-1" /> Refresh
                    </Button>
                    <Button variant="ghost" size="sm" onClick={handleExpandAll}>
                        Expand All
                    </Button>
                    <Button variant="ghost" size="sm" onClick={handleCollapseAll}>
                        Collapse All
                    </Button>
                </div>
            </header>
            {errorMsg && <ErrorBanner message={errorMsg} onClose={() => setErrorMsg(null)} />}
            {loading && (
                <div className="space-y-2">
                    {[...Array(6)].map((_, i) => (
                        <Skeleton key={i} className="h-4 w-full" />
                    ))}
                </div>
            )}
            {!loading && tree.length > 0 && (
                <TreeView nodes={tree} openValues={open} onOpenChange={setOpen} />
            )}
        </main>
    );
}