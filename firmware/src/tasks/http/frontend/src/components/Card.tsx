import { useState } from "preact/hooks";

export function Card(props: {
	title: string;
	children: preact.ComponentChildren;
}) {
	return (
		<div class="mb-4 rounded-xl border border-gray-700 bg-gray-800 p-4">
			<h2 class="mb-4 text-xl font-semibold">{props.title}</h2>
			{props.children}
		</div>
	);
}

export function FoldoutCard(props: {
	title: string;
	defaultOpen?: boolean;
	children: preact.ComponentChildren;
}) {
	const [open, setOpen] = useState(props.defaultOpen ?? false);

	return (
		<div class="mb-4 rounded-xl border border-gray-700 bg-gray-800">
			{/* Header */}
			<button
				type="button"
				class="flex w-full items-center justify-between p-4 text-left"
				onClick={() => setOpen(!open)}
			>
				<span class="text-lg font-semibold">{props.title}</span>

				<span class={`transition-transform ${open ? "rotate-90" : ""}`}>
					▶
				</span>
			</button>

			{/* Content */}
			<div
				class={`grid transition-all duration-200 ease-in-out bg-black/60 rounded-b-lg ${
					open
						? "grid-rows-[1fr] opacity-100"
						: "grid-rows-[0fr] opacity-0"
				}`}
			>
				<div class="overflow-hidden p-4">{props.children}</div>
			</div>
		</div>
	);
}
