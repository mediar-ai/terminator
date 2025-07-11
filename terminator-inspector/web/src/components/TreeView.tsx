"use client";

import * as Accordion from '@radix-ui/react-accordion';
import clsx from 'clsx';
import { ChevronDown } from 'lucide-react';
import { useId } from 'react';

export interface UIElementAttributes {
    role: string;
    name?: string;
    // value?: string;
}

export interface UINode {
    id?: string;
    attributes: UIElementAttributes;
    children?: UINode[];
}

interface TreeViewProps {
    nodes: UINode[];
    onHover?: (node: UINode) => void;
}

function Label({ node }: { node: UINode }) {
    const { attributes, id } = node;
    return (
        <span>
            {attributes?.name || attributes?.role || id || 'unknown'}
        </span>
    );
}

export default function TreeView({ nodes, onHover }: TreeViewProps) {
    // Radix Accordion expects IDs – we generate unique roots
    const rootId = useId();

    const renderNode = (node: UINode, path: string) => {
        const hasChildren = !!node.children && node.children.length > 0;
        const itemValue = `${path}`;

        return (
            <Accordion.Item key={itemValue} value={itemValue} className="pl-2">
                <Accordion.Header asChild>
                    <div
                        className={clsx(
                            'flex items-center gap-1 cursor-pointer select-none hover:text-blue-600',
                        )}
                        onMouseEnter={() => onHover?.(node)}
                    >
                        {hasChildren && (
                            <Accordion.Trigger
                                className="group data-[state=open]:rotate-180 transition-transform"
                            >
                                <ChevronDown size={14} className="text-gray-500" />
                            </Accordion.Trigger>
                        )}
                        <Label node={node} />
                    </div>
                </Accordion.Header>
                {hasChildren && (
                    <Accordion.Content className="pl-3 border-l border-gray-200">
                        {node.children!.map((child, idx) =>
                            renderNode(child, `${path}.${idx}`),
                        )}
                    </Accordion.Content>
                )}
            </Accordion.Item>
        );
    };

    return (
        <Accordion.Root type="multiple" className="text-sm">
            {nodes.map((node, idx) => renderNode(node, `${rootId}-${idx}`))}
        </Accordion.Root>
    );
}