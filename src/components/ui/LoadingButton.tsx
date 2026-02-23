import { Loader2 } from 'lucide-react';
import { ButtonHTMLAttributes, ReactNode } from 'react';

interface LoadingButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  isLoading: boolean;
  loadingText?: string;
  children: ReactNode;
  variant?: 'primary' | 'danger' | 'secondary';
  icon?: ReactNode;
}

export function LoadingButton({
  isLoading,
  loadingText,
  children,
  variant = 'primary',
  icon,
  className = '',
  disabled,
  ...props
}: LoadingButtonProps) {
  const baseStyles = 'w-full py-3 px-4 rounded-lg font-medium flex items-center justify-center gap-2 transition-colors disabled:opacity-50 disabled:cursor-not-allowed';
  
  const variantStyles = {
    primary: 'bg-blue-600 text-white hover:bg-blue-700',
    danger: 'bg-red-50 text-red-600 hover:bg-red-100',
    secondary: 'bg-gray-100 text-gray-800 hover:bg-gray-200',
  };

  return (
    <button
      className={`${baseStyles} ${variantStyles[variant]} ${className}`}
      disabled={disabled || isLoading}
      {...props}
    >
      {isLoading ? (
        <Loader2 className="w-5 h-5 animate-spin" />
      ) : icon ? (
        icon
      ) : null}
      {isLoading ? (loadingText || '处理中...') : children}
    </button>
  );
}
