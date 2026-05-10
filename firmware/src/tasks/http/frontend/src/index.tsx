import { render } from "preact";

import "virtual:uno.css";
import { Dashboard } from "./Dashboard";

export function App() {
	return Dashboard();
}

render(<App />, document.getElementById("app"));
