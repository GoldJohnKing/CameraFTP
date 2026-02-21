import React from "react";
import ReactDOM from "react-dom/client";
import { Toaster } from 'sonner';
import App from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
    <Toaster 
      position="top-center" 
      richColors 
      closeButton
      duration={4000}
    />
  </React.StrictMode>,
);