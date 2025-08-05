-- Add migration script here
CREATE TABLE users (
    id PRIMARY KEY INT GENERATED ALWAYS AS IDENTITY,
    email CITEXT UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    password TEXT NOT NULL,
);

CREATE TABLE devices (
    id PRIMARY KEY INT GENERATED ALWAYS AS IDENTITY,
    name TEXT,
    user_id INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE device_settings (
    id PRIMARY KEY INT GENERATED ALWAYS AS IDENTITY,
    device_id INT NOT NULL UNIQUE,
    FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);


CREATE TABLE notifications (
    id PRIMARY KEY INT GENERATED ALWAYS AS IDENTITY,
    user_id INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    device_id INT,
    priority INT NOT NULL DEFAULT 1, -- 1 (notice), 2 (regular), 3 (emergency)
    send_out BOOLEAN NOT NULL DEFAULT false,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE SET NULL
);
