"use client";

import { invoke } from '@tauri-apps/api/tauri';
import React, { useEffect, useState } from 'react';
import TreeView, { UINode } from '../components/TreeView';

export default function HomePage() {
    const [tree, setTree] = useState<UINode[]>([]);

    useEffect(() => {
        invoke('get_ui_tree')
            .then((res) => setTree(res as any[]))
            .catch((err) => console.error(err));
    }, []);

    return (
        <main className="p-4">
            <h1 className="text-2xl font-semibold mb-4">Terminator Inspector</h1>
            {tree.length === 0 ? (
                <p>Loading UI tree...</p>
            ) : (
                <TreeView
                    nodes={tree}
                    onHover={(node) =>
                        invoke('highlight_element', {
                            serialized: JSON.stringify(node),
                        })
                    }
                />
            )}
        </main>
    );
}