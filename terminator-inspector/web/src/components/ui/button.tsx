import { cn } from '@/lib/utils';
import { VariantProps, cva } from 'class-variance-authority';

const buttonVariants = cva(
    'inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2 disabled:opacity-50 disabled:pointer-events-none ring-offset-background',
    {
        variants: {
            variant: {
                default: 'bg-blue-600 text-white hover:bg-blue-700',
                outline: 'border border-input hover:bg-accent hover:text-accent-foreground',
                ghost: 'hover:bg-accent hover:text-accent-foreground',
            },
            size: {
                sm: 'h-8 px-3',
                md: 'h-10 px-4',
                lg: 'h-12 px-6',
            },
        },
        defaultVariants: {
            variant: 'default',
            size: 'md',
        },
    },
);

export interface ButtonProps
    extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> { }

export const Button = ({ className, variant, size, ...props }: ButtonProps) => {
    return (
        <button className={cn(buttonVariants({ variant, size }), className)} {...props} />
    );
};