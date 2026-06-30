# Discovery: Database

## Purpose

Detect the project's data model: entities, relationships, field types,
and migration patterns.

## Scan Targets

| Pattern | What It Reveals |
|---------|----------------|
| Django: `models.py` files | ORM models, fields, relationships |
| Rails: `app/models/*.rb` | ActiveRecord models |
| Prisma: `schema.prisma` | Prisma schema |
| TypeORM: `*.entity.ts` | TypeORM entities |
| Sequelize: `models/*.js` | Sequelize models |
| Go: `*_model.go` | Go struct definitions |
| SQL: `migrations/` / `schema.sql` | Raw schema |
| MongoDB: Mongoose schemas | Document models |

## Tools

Use Grep to find model/entity definitions, Read to parse them.

## Output

Structured report:
- Entity inventory (name, field count, key relationships)
- Database type (PostgreSQL, MySQL, MongoDB, SQLite, etc.)
- ORM/query layer in use
- Migration framework and directory
- Seed data presence
