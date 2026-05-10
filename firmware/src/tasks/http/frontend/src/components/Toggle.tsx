export function Toggle(props: {
	checked: boolean;
	onChange: (v: boolean) => void;
}) {
	return (
		<button
			class={`rounded px-4 py-2 ${
				props.checked ? "bg-green-600" : "bg-gray-600"
			}`}
			onClick={() => props.onChange(!props.checked)}
		>
			{props.checked ? "ON" : "OFF"}
		</button>
	);
}
