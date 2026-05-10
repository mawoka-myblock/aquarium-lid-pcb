export class Api {
	constructor() {}
	private static async get<T>(url: string): Promise<T> {
		const r = await fetch(url);
		if (!r.ok) throw new Error(url);
		return r.json();
	}

	private static async post(url: string, body: unknown) {
		const r = await fetch(url, {
			method: "POST",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify(body),
		});

		if (!r.ok) throw new Error(url);
	}

	static async get_config(): Promise<Config> {
		if (import.meta.env.PROD) {
			return await Api.get("/api/config");
		}
		return {
			fan_on_threshold: 25.3,
			fan_off_threshold: 25.0,
			max_safe_temp: 26.0,
			min_safe_temp: 20.0,
			alarm_hysteresis: 0.3,
			led_brightness: 255,
		};
	}

	static async get_fan(): Promise<BoolState> {
		if (import.meta.env.PROD) {
			return await Api.get("/api/fan");
		}
		return { on: true };
	}

	static async get_buzzer(): Promise<BoolState> {
		if (import.meta.env.PROD) {
			return await Api.get("/api/buzzer");
		}
		return { on: false };
	}

	static async get_water_temp(): Promise<WaterTemp> {
		if (import.meta.env.PROD) {
			return await Api.get("/api/data/water");
		}
		return {
			temp: 23.25654984651,
		};
	}

	static async post_config(cfg: Config) {
		await Api.post("/api/config", cfg);
	}

	static async post_fan(d: BoolState) {
		await Api.post("/api/fan", d);
	}

	static async post_buzzer(d: BoolState) {
		await Api.post("/api/buzzer", d);
	}

	static async post_mqtt(cfg: MQTTConfig) {
		await Api.post("/api/mqtt", cfg);
	}
}

export type Config = {
	fan_on_threshold: number;
	fan_off_threshold: number;
	max_safe_temp: number;
	min_safe_temp: number;
	alarm_hysteresis: number;
	led_brightness: number;
};

export type BoolState = {
	on: boolean;
};

export type WaterTemp = {
	temp: number | null;
};

export type MQTTConfig = {
	host: string;
	port?: number;
	client_id: string;
	username: string;
	password: string;
};
