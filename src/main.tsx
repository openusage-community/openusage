import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { installFrontendErrorLogging } from "@/lib/frontend-error-logging";
import "./index.css";

installFrontendErrorLogging();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
