import { AlertCircle } from 'lucide-react';

interface ErrorMessageProps {
  message: string | null;
  onDismiss?: () => void;
  variant?: 'inline' | 'card';
}

export function ErrorMessage({ message, onDismiss, variant = 'inline' }: ErrorMessageProps) {
  if (!message) return null;

  if (variant === 'card') {
    return (
      <div className="p-3 bg-red-50 border border-red-200 rounded-lg flex items-start gap-2">
        <AlertCircle className="w-5 h-5 text-red-500 flex-shrink-0 mt-0.5" />
        <div className="flex-1">
          <p className="text-sm text-red-600">{message}</p>
        </div>
        {onDismiss && (
          <button
            onClick={onDismiss}
            className="text-red-400 hover:text-red-600 text-sm"
          >
            ✕
          </button>
        )}
      </div>
    );
  }

  return (
    <p className="mt-3 text-sm text-red-600 text-center flex items-center justify-center gap-1">
      <AlertCircle className="w-4 h-4" />
      {message}
    </p>
  );
}
