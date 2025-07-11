"use client";

import { invoke } from '@tauri-apps/api/tauri';
import { useEffect, useState } from 'react';

export default function HomePage() {
    const [tree, setTree] = useState<any[]>([]);

    useEffect(() => {
        invoke('get_ui_tree')
            .then((res) => setTree(res as any[]))
            .catch((err) => console.error(err));
    }, []);

    const renderNode = (node: any, depth = 0) => {
        const label =
            node.attributes?.name ??
            node.attributes?.role ??
            node.id ??
            'unknown';

        return (
            <div key={Math.random()} style={{ marginLeft: depth * 8 }}>
                <span
                    onMouseEnter={() =>
                        invoke('highlight_element', {
                            serialized: JSON.stringify(node),
                        })
                    }
                    className="cursor-pointer hover:text-blue-600"
                >
                    {label}
                </span>
                {node.children?.map((child: any) => renderNode(child, depth + 1))}
            </div>
        );
    };

    return (
        <main className="p-4">
            <h1 className="text-2xl font-semibold mb-4">Terminator Inspector</h1>
            {tree.length === 0 ? (
                <p>Loading UI tree...</p>
            ) : (
                <div className="text-sm">{tree.map((n) => renderNode(n))}</div>
            )}
        </main>
    );
}