import { useEffect, useState } from "preact/hooks";
import { Api } from "./api";
import type { Config, MQTTConfig } from "./api";
import { Card, FoldoutCard } from "./components/Card";
import { Toggle } from "./components/Toggle";
import { NumberInput, TextInput } from "./components/Inputs";
import { Settings } from "./Settings";

export function Dashboard() {
	const [fan, setFan] = useState(false);
	const [buzzer, setBuzzer] = useState(false);
	const [waterTemp, setWaterTemp] = useState<number | null>(null);

	useEffect(() => {
		let running = false;
		const poll = async () => {
			console.log("Update data!");
			if (running) return;
			running = true;
			try {
				setFan((await Api.get_fan()).on);
				setBuzzer((await Api.get_buzzer()).on);
				setWaterTemp(
					Math.round((await Api.get_water_temp()).temp * 10) / 10,
				);
			} catch (e) {
				console.error(e);
			} finally {
				running = false;
			}
		};
		poll();
		const interval = setInterval(poll, 5000);
		return () => clearInterval(interval);
	}, []);

	return (
		<div class="p-4 text-white font-sans">
			<h1 class="mb-4 text-2xl">Aquarium</h1>

			<Card title="Status">
				<div>Water: {waterTemp ?? "--"} °C</div>
			</Card>

			<Card title="Controls">
				<div class="flex flex-row justify-around">
					<div class="flex flex-col">
						<p class="mx-auto">Fan</p>
						<Toggle
							checked={fan}
							onChange={async (v) => {
								setFan(v);
								await Api.post_fan({ on: v });
							}}
						/>
					</div>
					<div class="flex flex-col">
						<p class="mx-auto">Buzzer</p>
						<Toggle
							checked={buzzer}
							onChange={async (v) => {
								setBuzzer(v);
								await Api.post_buzzer({ on: v });
							}}
						/>
					</div>
				</div>
			</Card>

			<Settings />
		</div>
	);
}
