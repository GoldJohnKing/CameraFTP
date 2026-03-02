import { StrictMode } from "react";
import ReactDOM from "react-dom/client";
import { Toaster } from 'sonner';
import App from "./App";
import { ErrorBoundary } from "./components/ui";
import "./index.css";

// Disable right-click context menu
document.addEventListener('contextmenu', (e) => e.preventDefault());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    <ErrorBoundary>
      <App />
      <Toaster 
        position="bottom-center" 
        richColors 
        closeButton
        duration={4000}
      />
    </ErrorBoundary>
  </StrictMode>,
);