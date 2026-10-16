import { MigrationInterface, QueryRunner, TableColumn, TableIndex } from 'typeorm';

/**
 * Adds `apiKeyLookupHash` to the `merchants` table.
 *
 * This column stores the SHA-256 hex digest of the raw API key, enabling an
 * indexed O(1) lookup instead of the previous O(N) bcrypt scan across all
 * merchants (resolves issue #5).
 *
 * The bcrypt hash (`apiKeyHash`) is still verified after the index lookup;
 * `apiKeyLookupHash` is only used for record selection.
 *
 * Existing rows will have NULL in this column until their API keys are rotated.
 * Authentication for those merchants will fall back to returning null (key not
 * found), which is equivalent to treating them as having no API key until the
 * key is re-issued through the normal rotation flow.
 */
export class AddMerchantApiKeyLookupHash1729082844000 implements MigrationInterface {
  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.addColumn(
      'merchants',
      new TableColumn({
        name: 'apiKeyLookupHash',
        type: 'varchar',
        isNullable: true,
        default: null,
      }),
    );

    await queryRunner.createIndex(
      'merchants',
      new TableIndex({
        name: 'IDX_merchants_apiKeyLookupHash',
        columnNames: ['apiKeyLookupHash'],
      }),
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.dropIndex('merchants', 'IDX_merchants_apiKeyLookupHash');
    await queryRunner.dropColumn('merchants', 'apiKeyLookupHash');
  }
}
