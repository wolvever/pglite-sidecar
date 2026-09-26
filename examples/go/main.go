// Command pglite-go-example connects to the pglite-sidecar Unix-socket URI
// in DATABASE_URL and runs a small create / insert / select round-trip.
package main

import (
	"context"
	"fmt"
	"os"
	"time"

	"github.com/jackc/pgx/v5"
)

func main() {
	if err := run(); err != nil {
		fmt.Fprintf(os.Stderr, "pglite-go-example: %v\n", err)
		os.Exit(1)
	}
}

func run() error {
	uri := os.Getenv("DATABASE_URL")
	if uri == "" {
		return fmt.Errorf("DATABASE_URL is required (use the URI printed on stdout by pglite-sidecar)")
	}

	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()

	conn, err := pgx.Connect(ctx, uri)
	if err != nil {
		return fmt.Errorf("connect: %w", err)
	}
	defer conn.Close(context.Background())

	var one int
	if err := conn.QueryRow(ctx, "SELECT 1").Scan(&one); err != nil {
		return fmt.Errorf("select 1: %w", err)
	}
	if one != 1 {
		return fmt.Errorf("select 1: got %d", one)
	}

	if _, err := conn.Exec(ctx, `
		CREATE TABLE IF NOT EXISTS greetings (
			id serial PRIMARY KEY,
			msg text NOT NULL
		)
	`); err != nil {
		return fmt.Errorf("create table: %w", err)
	}

	if _, err := conn.Exec(ctx, `INSERT INTO greetings (msg) VALUES ($1)`, "hello from go"); err != nil {
		return fmt.Errorf("insert: %w", err)
	}

	var msg string
	if err := conn.QueryRow(ctx, `SELECT msg FROM greetings ORDER BY id DESC LIMIT 1`).Scan(&msg); err != nil {
		return fmt.Errorf("select greeting: %w", err)
	}

	fmt.Printf("ok: connected via pgx, SELECT 1 = %d, latest greeting = %q\n", one, msg)
	return nil
}
