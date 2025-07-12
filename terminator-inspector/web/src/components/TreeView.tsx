"use client";

import {
    Accordion,
    AccordionContent,
    AccordionItem,
    AccordionTrigger,
} from '@/components/ui/accordion';
import { invoke } from '@tauri-apps/api/tauri';
import { useState } from 'react';

export interface UIElementAttributes {
    role: string;
    name?: string;
}

export interface SerializableNode {
    id?: string;
    role: string;
    name?: string;
    children?: SerializableNode[];
}

interface TreeViewProps {
    nodes: SerializableNode[];
}

export default function TreeView({ nodes }: TreeViewProps) {
    const [hoverPath, setHoverPath] = useState<string | null>(null);
    const renderNode = (node: SerializableNode, path: string) => {
        const hasChildren = !!node.children && node.children.length > 0;
        const label = node.name || node.role || node.id || 'unknown';

        return (
            <AccordionItem key={path} value={path} className="pl-2">
                <AccordionTrigger
                    className="text-left"
                    onMouseEnter={() =>
                        invoke('highlight_element', {
                            serialized: JSON.stringify(node),
                            color: 0xff0000,
                        })
                            .catch(console.error)
                            .finally(() => setHoverPath(path))
                    }
                    onMouseLeave={() => setHoverPath(null)}
                >
                    <span className="font-medium mr-2">{node.name ?? '—'}</span>
                    {node.role && (
                        <span className="text-xs bg-muted rounded px-1 py-0.5 mr-1 capitalize">
                            {node.role}
                        </span>
                    )}
                    {node.id && <span className="text-xs text-gray-500">#{node.id}</span>}
                </AccordionTrigger>
                {hasChildren && (
                    <AccordionContent className="pl-4 border-l border-muted">
                        <Accordion
                            type="multiple"
                            className="space-y-1"
                        >
                            {node.children!.map((child, idx) =>
                                renderNode(child, `${path}.${idx}`),
                            )}
                        </Accordion>
                    </AccordionContent>
                )}
            </AccordionItem>
        );
    };

    return (
        <Accordion type="multiple" className="text-sm">
            {nodes.map((n, idx) => renderNode(n, `${idx}`))}
        </Accordion>
    );
}