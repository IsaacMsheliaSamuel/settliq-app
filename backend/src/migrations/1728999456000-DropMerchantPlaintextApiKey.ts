import { MigrationInterface, QueryRunner, TableColumn } from 'typeorm';

/**
 * Drops the plaintext `apiKey` column from the `merchants` table.
 *
 * Raw API keys must never be stored at rest; only the bcrypt hash (`apiKeyHash`)
 * is persisted. This migration removes the insecure column that was introduced
 * before that policy was enforced (resolves issue #30).
 */
export class DropMerchantPlaintextApiKey1728999456000 implements MigrationInterface {
  public async up(queryRunner: QueryRunner): Promise<void> {
    const table = await queryRunner.getTable('merchants');
    if (table?.findColumnByName('apiKey')) {
      await queryRunner.dropColumn('merchants', 'apiKey');
    }
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.addColumn(
      'merchants',
      new TableColumn({
        name: 'apiKey',
        type: 'varchar',
        isNullable: true,
      }),
    );
  }
}
